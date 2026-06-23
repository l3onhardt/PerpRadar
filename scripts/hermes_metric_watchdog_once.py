#!/usr/bin/env python3
import json, os, subprocess, time, urllib.request, urllib.parse, datetime, pathlib, signal, statistics, base64
from collections import defaultdict

ROOT = pathlib.Path('/home/winsleyboursiquot433/PerpRadar')
REPORT_DIR = ROOT / 'reports'
REPORT_DIR.mkdir(parents=True, exist_ok=True)
LATEST = REPORT_DIR / 'watchdog_latest.json'
HISTORY = REPORT_DIR / 'watchdog_history.jsonl'
API = 'http://127.0.0.1:18080'
CH = 'http://127.0.0.1:8123'
SYMS = ['BTCUSDT','ETHUSDT','SOLUSDT']

def now(): return datetime.datetime.now(datetime.timezone.utc).isoformat()

def run(cmd, timeout=30, cwd=None):
    try:
        p = subprocess.run(cmd, shell=True, cwd=str(cwd) if cwd else None, text=True,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
        return {'cmd': cmd, 'rc': p.returncode, 'stdout': p.stdout[-5000:], 'stderr': p.stderr[-5000:]}
    except subprocess.TimeoutExpired as e:
        return {'cmd': cmd, 'rc': 124, 'stdout': (e.stdout or '')[-5000:] if isinstance(e.stdout, str) else '', 'stderr': (e.stderr or '')[-5000:] if isinstance(e.stderr, str) else 'timeout'}
    except Exception as e:
        return {'cmd': cmd, 'rc': 999, 'stdout': '', 'stderr': repr(e)}

def http_json(url, timeout=5):
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            body = r.read().decode('utf-8', 'replace')
        return True, json.loads(body), body[:1000]
    except Exception as e:
        return False, None, repr(e)

def ch_query(sql):
    data = sql.encode('utf-8')
    req = urllib.request.Request(CH, data=data, method='POST')
    req.add_header('Authorization', 'Basic ' + base64.b64encode(b'perp_radar:perp_radar').decode('ascii'))
    try:
        with urllib.request.urlopen(req, timeout=8) as r:
            body = r.read().decode('utf-8', 'replace')
        return {'ok': True, 'raw': body[-4000:]}
    except Exception as e:
        return {'ok': False, 'raw': repr(e)}

def proc_checks():
    return {
        'ps_perp_radar': run("ps -eo pid,ppid,etime,cmd | grep -E '[/ ]perp-radar($| )|target/release/perp-radar' | grep -v grep || true"),
        'ps_trader_runner': run("ps -eo pid,ppid,etime,cmd | grep -E 'perp_radar_ai_trader\\.runner|live_canary_events|ai_trader_events' | grep -v grep || true"),
        'ss_18080': run("ss -ltnp 2>/dev/null | grep -E ':18080' || true"),
    }

def sample_packets(seconds=72, interval=6):
    samples=[]
    deadline=time.time()+seconds
    while time.time() <= deadline:
        row={'sample_ts': now(), 'symbols': {}}
        for sym in SYMS:
            ok, obj, raw = http_json(f'{API}/v1/packet/{sym}', timeout=4)
            ent={'ok': ok}
            if ok and isinstance(obj, dict):
                q=obj.get('quality') or {}; price=obj.get('price') or {}; chart=obj.get('chart') or {}; liq=obj.get('liquidity') or {}
                ent.update({
                    'ts': obj.get('ts') or obj.get('timestamp'),
                    'last': price.get('last'),
                    'ema_20': chart.get('ema_20'),
                    'signature': chart.get('signature'),
                    'liq_5bp_usd': liq.get('liq_5bp_usd'),
                    'i5': liq.get('i5'),
                    'freshness_ms': q.get('freshness_ms'),
                    'book_seq_ok': q.get('book_seq_ok'),
                    'stale': q.get('stale'),
                })
            else:
                ent['error']=raw
            row['symbols'][sym]=ent
        samples.append(row)
        if time.time()+interval <= deadline:
            time.sleep(interval)
        else:
            break
    return samples

def movement(vals):
    clean=[v for v in vals if v is not None]
    return len(set(map(str, clean))) > 1

def assess_samples(samples):
    ass={}
    ok_all=True
    for sym in SYMS:
        ents=[s['symbols'].get(sym,{}) for s in samples]
        oks=[e.get('ok') for e in ents]
        freshness=[e.get('freshness_ms') for e in ents if isinstance(e.get('freshness_ms'), (int,float))]
        stale_bad=[e for e in ents if e.get('stale') is True]
        seq_bad=[e for e in ents if e.get('book_seq_ok') is False]
        ts_move=movement([e.get('ts') for e in ents])
        last_move=movement([e.get('last') for e in ents])
        liq_move=movement([e.get('liq_5bp_usd') for e in ents]) or movement([e.get('i5') for e in ents])
        chart_move=movement([e.get('ema_20') for e in ents]) or movement([e.get('signature') for e in ents])
        fresh_ok=bool(freshness) and max(freshness) < 15000 and (statistics.median(freshness) < 5000 if freshness else False)
        sym_ok=all(oks) and fresh_ok and not stale_bad and not seq_bad and ts_move and liq_move
        ass[sym]={
            'ok': sym_ok,
            'samples': len(ents),
            'all_http_ok': all(oks),
            'freshness_max_ms': max(freshness) if freshness else None,
            'freshness_median_ms': statistics.median(freshness) if freshness else None,
            'stale_count': len(stale_bad),
            'book_seq_bad_count': len(seq_bad),
            'timestamp_moved': ts_move,
            'price_moved': last_move,
            'liquidity_moved': liq_move,
            'chart_moved': chart_move,
            'first': ents[0] if ents else None,
            'last': ents[-1] if ents else None,
        }
        ok_all = ok_all and sym_ok
    return ok_all, ass

def clickhouse_checks():
    checks={}
    latest_sql = "SELECT symbol, max(ts) FROM perp_radar.latest_packets WHERE symbol IN ('BTCUSDT','ETHUSDT','SOLUSDT') GROUP BY symbol FORMAT JSONEachRow"
    feat_sql = "SELECT symbol, count(), uniqExact(price), uniqExact(spread_bp), uniqExact(i5), uniqExact(lri), max(ts) FROM perp_radar.features_1m WHERE symbol IN ('BTCUSDT','ETHUSDT','SOLUSDT') AND ts > now() - INTERVAL 30 MINUTE GROUP BY symbol FORMAT JSONEachRow"
    checks['latest_packets'] = ch_query(latest_sql)
    checks['features_1m'] = ch_query(feat_sql)
    checks['ok'] = checks['latest_packets']['ok'] and checks['features_1m']['ok']
    return checks

def health():
    ok,obj,raw=http_json(API+'/v1/health', timeout=5)
    return {'ok': ok and obj == {'ok': True}, 'response': obj if ok else raw}

def stop_service(proc_text):
    actions=[]
    pids=[]
    for line in proc_text.splitlines():
        parts=line.split(None, 4)
        if parts and parts[0].isdigit():
            cmd=parts[-1] if len(parts)>=5 else line
            if 'perp-radar' in cmd and 'perp_radar_ai_trader.runner' not in cmd:
                pids.append(int(parts[0]))
    for pid in pids:
        try:
            os.kill(pid, signal.SIGTERM); actions.append(f'SIGTERM perp-radar pid {pid}')
        except Exception as e: actions.append(f'failed SIGTERM {pid}: {e!r}')
    time.sleep(5)
    for pid in pids:
        try:
            os.kill(pid, 0)
            os.kill(pid, signal.SIGKILL); actions.append(f'SIGKILL perp-radar pid {pid}')
        except ProcessLookupError: pass
        except Exception as e: actions.append(f'failed SIGKILL check {pid}: {e!r}')
    return actions

def start_service():
    actions=[]
    binp=ROOT/'target/release/perp-radar'
    if not binp.exists():
        actions.append('release binary missing; cargo build --release')
        actions.append(run('cargo build --release', timeout=600, cwd=ROOT))
    env=os.environ.copy()
    env.update({
        'RUST_LOG':'info',
        'PERP_RADAR__API__BIND':'127.0.0.1:18080',
        'PERP_RADAR__BINANCE__PUBLIC_WS_BASE':'wss://fstream.binance.com',
        'PERP_RADAR__STORAGE__CLICKHOUSE_URL':'http://perp_radar:perp_radar@127.0.0.1:8123',
    })
    logdir=ROOT/'logs'; logdir.mkdir(exist_ok=True)
    log=open(logdir/'perp-radar-watchdog.log','ab')
    p=subprocess.Popen([str(binp)], cwd=str(ROOT), env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    actions.append(f'started perp-radar pid {p.pid} with explicit env overrides')
    time.sleep(12)
    return actions

def do_check(label):
    pc=proc_checks(); h=health(); samples=sample_packets(); fresh_ok, fresh=assess_samples(samples); ch=clickhouse_checks()
    ss_has=':18080' in pc['ss_18080'].get('stdout','')
    return {'label': label, 'processes': pc, 'api_health': h, 'sample_assessment': fresh, 'freshness_ok': fresh_ok, 'clickhouse': ch, 'listener_ok': ss_has, 'healthy': h['ok'] and fresh_ok and ch['ok'] and ss_has, 'raw_sample_count': len(samples), 'raw_samples_tail': samples[-2:]}

def main():
    report={'watchdog':'perp-radar-metric-service','project':'PerpRadar-AI-Trader-v11','started_at':now(),'actions_taken':[]}
    initial=do_check('initial')
    report['initial']=initial
    if initial['healthy']:
        report['status']='OK'; report['actions_taken'].append('checked process/listener/API/packet freshness/ClickHouse; no changes')
    else:
        report['actions_taken'].append('initial check failed; attempting metric-service-only repair')
        # test if practical before restart
        report['actions_taken'].append({'cargo_test_before_restart': run('cargo test --workspace --all-targets', timeout=600, cwd=ROOT)})
        report['actions_taken'].extend(stop_service(initial['processes']['ps_perp_radar'].get('stdout','')))
        report['actions_taken'].extend(start_service())
        after=do_check('after_repair')
        report['after_repair']=after
        if after['healthy']:
            report['status']='REPAIRED'
        else:
            report['status']='ALERT'
            report['root_cause_guess']='metric service/API freshness/ClickHouse check still failing after safe restart; inspect logs/perp-radar-watchdog.log and report details'
    report['finished_at']=now()
    tmp = json.dumps(report, ensure_ascii=False, indent=2, default=str)
    LATEST.write_text(tmp + '\n')
    with HISTORY.open('a') as f:
        f.write(json.dumps({'ts': report['finished_at'], 'status': report['status'], 'latest_report': str(LATEST), 'healthy_initial': initial['healthy'], 'actions_taken': report['actions_taken']}, ensure_ascii=False, default=str) + '\n')
    print(json.dumps({'status': report['status'], 'actions_taken': report['actions_taken'], 'local_report_path': str(LATEST)}, ensure_ascii=False, default=str))

if __name__ == '__main__':
    main()
