#!/usr/bin/env python3
import json, subprocess, time, urllib.request, urllib.parse, os, datetime, statistics, pathlib, sys
BASE='http://127.0.0.1:18080'
SYMS=['BTCUSDT','ETHUSDT','SOLUSDT']
root=pathlib.Path('/home/winsleyboursiquot433/PerpRadar')
reports=root/'reports'; reports.mkdir(parents=True, exist_ok=True)
latest=reports/'watchdog_latest.json'; hist=reports/'watchdog_history.jsonl'
started=datetime.datetime.now(datetime.timezone.utc)
cmd_results=[]
def run(cmd, timeout=20):
    try:
        p=subprocess.run(cmd, shell=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
        out=p.stdout.strip(); rc=p.returncode
    except Exception as e:
        out=repr(e); rc=999
    cmd_results.append({'cmd':cmd,'returncode':rc,'output':out[-4000:]})
    return rc,out

def sample_packets(rounds=15, sleep_sec=5):
    samples=[]; errors=[]
    for i in range(rounds):
        row={'at':datetime.datetime.now(datetime.timezone.utc).isoformat()}
        for sym in SYMS:
            try:
                with urllib.request.urlopen(f'{BASE}/v1/packet/{sym}', timeout=4) as r:
                    p=json.loads(r.read().decode())
                row[sym]={
                    'ts':p.get('ts') or p.get('timestamp'),
                    'price_last':(p.get('price') or {}).get('last'),
                    'ema_20':(p.get('chart') or {}).get('ema_20'),
                    'signature':(p.get('chart') or {}).get('signature'),
                    'liq_5bp_usd':(p.get('liquidity') or {}).get('liq_5bp_usd'),
                    'i5':(p.get('liquidity') or {}).get('i5'),
                    'freshness_ms':(p.get('quality') or {}).get('freshness_ms'),
                    'book_seq_ok':(p.get('quality') or {}).get('book_seq_ok'),
                    'stale':(p.get('quality') or {}).get('stale'),
                }
            except Exception as e:
                errors.append({'sym':sym,'at':row['at'],'error':repr(e)})
                row[sym]={'error':repr(e)}
        samples.append(row)
        if i < rounds-1: time.sleep(sleep_sec)
    return samples, errors

def eval_samples(samples):
    out={}; ok_all=True
    for sym in SYMS:
        vals=[s.get(sym,{}) for s in samples if 'error' not in s.get(sym,{})]
        if not vals:
            out[sym]={'ok':False,'reason':'no_valid_samples'}; ok_all=False; continue
        def uniq(field):
            return {json.dumps(v.get(field), sort_keys=True) for v in vals if v.get(field) is not None}
        fresh=[v.get('freshness_ms') for v in vals if isinstance(v.get('freshness_ms'), (int,float))]
        ts_moves=len(uniq('ts'))>=2
        depth_moves=(len(uniq('liq_5bp_usd'))>=2) or (len(uniq('i5'))>=2)
        price_moves=len(uniq('price_last'))>=2
        chart_moves=(len(uniq('ema_20'))>=2) or (len(uniq('signature'))>=2)
        fresh_good=bool(fresh) and statistics.median(fresh)<5000 and max(fresh[-3:])<15000
        seq_good=all(v.get('book_seq_ok') is True for v in vals[-5:])
        stale_good=all(v.get('stale') is False for v in vals[-5:])
        ok=fresh_good and seq_good and stale_good and ts_moves and depth_moves
        out[sym]={'ok':ok,'median_freshness_ms':statistics.median(fresh) if fresh else None,'max_recent_freshness_ms':max(fresh[-3:]) if fresh else None,'ts_moves':ts_moves,'price_moves':price_moves,'depth_moves':depth_moves,'chart_moves':chart_moves,'latest':vals[-1],'unique_counts':{k:len(uniq(k)) for k in ['ts','price_last','ema_20','signature','liq_5bp_usd','i5']}}
        ok_all = ok_all and ok
    return ok_all,out

def clickhouse_check():
    results={}; ok=True
    queries={
      'latest_packets_max_ts': "SELECT symbol, max(ts) AS max_ts, count() FROM perp_radar.latest_packets WHERE symbol IN ('BTCUSDT','ETHUSDT','SOLUSDT') GROUP BY symbol FORMAT JSONCompact",
      'features_1m_recent': "SELECT symbol, count(), uniqExact(price), uniqExact(spread_bp), uniqExact(i5), uniqExact(lri), max(ts) FROM perp_radar.features_1m WHERE symbol IN ('BTCUSDT','ETHUSDT','SOLUSDT') AND ts > now() - INTERVAL 10 MINUTE GROUP BY symbol FORMAT JSONCompact"
    }
    for name,q in queries.items():
        url='http://127.0.0.1:8123/?user=perp_radar&password=perp_radar&query='+urllib.parse.quote(q)
        try:
            with urllib.request.urlopen(url, timeout=8) as r: body=r.read().decode()
            parsed=json.loads(body); results[name]={'ok':True,'data':parsed.get('data'), 'meta':parsed.get('meta')}
        except Exception as e:
            results[name]={'ok':False,'error':repr(e)}; ok=False
    try:
        if len(results.get('latest_packets_max_ts',{}).get('data') or []) < 3:
            ok=False; results['latest_packets_eval']='missing_symbols_or_rows'
        data2=results.get('features_1m_recent',{}).get('data') or []
        if len(data2)<3:
            ok=False; results['features_1m_eval']='missing_recent_symbol_rows'
        else:
            bad=[r for r in data2 if int(r[1]) < 2 or int(r[2]) < 1 or int(r[3]) < 1 or int(r[4]) < 1]
            if bad: ok=False; results['features_1m_eval']={'bad_rows':bad}
    except Exception as e:
        ok=False; results['eval_error']=repr(e)
    return ok, results

def main():
    mode=sys.argv[1] if len(sys.argv)>1 else 'check'
    run("ps -eo pid,ppid,etime,cmd | grep -E 'perp-radar|perp_radar_ai_trader\\.runner|paper_runner|runner\\.py|live_canary_events|ai_trader_events' | grep -v grep || true")
    run("ss -ltnp 2>/dev/null | grep -E ':18080' || true")
    health_ok=False; health_body=''
    try:
        with urllib.request.urlopen(BASE+'/v1/health', timeout=4) as r:
            health_body=r.read().decode(); health_ok=json.loads(health_body).get('ok') is True
    except Exception as e: health_body=repr(e)
    samples, sample_errors = sample_packets()
    freshness_ok, freshness_eval = eval_samples(samples)
    ch_ok, ch_results = clickhouse_check()
    actions=['checked process/listener/API health, 70s packet freshness, ClickHouse persistence; no changes']
    status='OK' if (health_ok and freshness_ok and ch_ok) else 'ALERT'
    if mode=='postrepair' and health_ok and freshness_ok:
        status='REPAIRED' if ch_ok else 'ALERT'
        actions=['restarted metric service earlier; post-repair freshness and persistence rechecked']
    report={'status':status,'project':'PerpRadar-AI-Trader-v11 metric-service watchdog','started_at':started.isoformat(),'finished_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'actions_taken':actions,'health':{'ok':health_ok,'body':health_body},'freshness_ok':freshness_ok,'freshness_eval':freshness_eval,'sample_errors':sample_errors[-10:],'clickhouse_ok':ch_ok,'clickhouse':ch_results,'need_metric_service_restart': (not health_ok or not freshness_ok),'process_checks':cmd_results,'local_report_path':str(latest)}
    latest.write_text(json.dumps(report, ensure_ascii=False, indent=2))
    with hist.open('a') as f: f.write(json.dumps({'ts':report['finished_at'],'status':status,'freshness_ok':freshness_ok,'clickhouse_ok':ch_ok,'need_metric_service_restart':report['need_metric_service_restart']}, ensure_ascii=False)+'\n')
    print(json.dumps({'status':status,'need_metric_service_restart':report['need_metric_service_restart'],'actions_taken':actions,'local_report_path':str(latest)}, ensure_ascii=False))
if __name__=='__main__': main()
