# Replay: after a whole-file read that led to an edit, would Jev's window selection have kept the edited lines, and how many bytes would it drop?
import os
import json, re, glob, time, hashlib, math, statistics, requests
OUT = os.environ.get("REPLAY_OUT", ".")
key = os.environ.get("TYPESAFE_API_KEY")  # export it first; never commit it
URL = "https://api.typesafe.ai/v1/systemone"; H = {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}
roots = ["/home/emiliano/.claude/projects/-home-emiliano-Projects-ICG-icg-cloud", "/home/emiliano/.claude/projects/-home-emiliano-Projects-Prokey-arca-api"]
files = []
for r in roots: files += glob.glob(f"{r}/*.jsonl") + glob.glob(f"{r}/*/subagents/agent-*.jsonl")
SKIP_EXT = (".png", ".jpg", ".jpeg", ".pdf", ".svg", ".lock", ".json", ".min.js")

def text_of(content):
    if isinstance(content, str): return content
    if isinstance(content, list): return "\n".join(c.get("text", "") for c in content if isinstance(c, dict) and c.get("type") == "text")
    return ""

def norm_read(txt):
    txt = re.split(r'\n?<system-reminder>', txt)[0]
    lines = txt.split("\n")
    if sum(1 for l in lines[:20] if re.match(r'^\s*\d+\t', l)) >= 10:
        lines = [re.sub(r'^\s*\d+\t', '', l) for l in lines]
    return lines

pairs = []
for f in files:
    events = []  # (kind, path, content, intent)
    uses = {}; assistant_texts = []; task = None
    for line in open(f):
        try: j = json.loads(line)
        except: continue
        msg = j.get("message") or {}; content = msg.get("content"); role = msg.get("role")
        if role == "user" and task is None:
            t = content if isinstance(content, str) else text_of(content)
            if t.strip(): task = t[:1500]
        if not isinstance(content, list): continue
        if role == "assistant":
            t = text_of(content)
            if t.strip(): assistant_texts.append(t)
        for c in content:
            if c.get("type") == "tool_use":
                inp = c.get("input") or {}; name = c.get("name")
                path = None; kind = None
                if name == "Read" and "offset" not in inp and "limit" not in inp: path, kind = inp.get("file_path"), "read"
                elif name == "Bash":
                    m = re.match(r'^\s*cat\s+(?:-n\s+)?([^\s|;&><`$]+)\s*$', inp.get("command", ""))
                    if m: path, kind = m.group(1), "cat"
                elif name in ("Edit", "MultiEdit", "Write"):
                    path, kind = inp.get("file_path"), "edit"
                if path:
                    uses[c["id"]] = (kind, path, inp, "\n\n".join(assistant_texts[-2:])[-2500:])
                    if kind == "edit":
                        old = inp.get("old_string") if name == "Edit" else (inp.get("edits", [{}])[0].get("old_string") if name == "MultiEdit" else None)
                        events.append(("edit", path, old, None))
            elif c.get("type") == "tool_result" and c.get("tool_use_id") in uses:
                kind, path, inp, intent = uses.pop(c["tool_use_id"])
                if kind in ("read", "cat") and not c.get("is_error"):
                    events.append(("read", path, text_of(c.get("content")), intent))
    for i, (k, path, content, intent) in enumerate(events):
        if k != "read" or path.endswith(SKIP_EXT): continue
        for k2, p2, old, _ in events[i+1:i+16]:
            if k2 == "read" and p2 == path: break  # a re-read supersedes
            if k2 == "edit" and p2 == path and old:
                pairs.append({"file": f, "path": path, "content": content, "intent": intent, "task": task or "", "old": old})
                break

print("pairs found", len(pairs))
pairs.sort(key=lambda p: hashlib.md5((p["file"] + p["path"] + p["old"][:50]).encode()).hexdigest())
rows = []; raw = []
def call(state, questions):
    for attempt in range(4):
        t = time.perf_counter()
        r = requests.post(URL, headers=H, json={"state": state, "model": "jev-latest", "questions": questions}, timeout=90)
        dt = time.perf_counter() - t
        if r.status_code in (429, 529): time.sleep(1.5 * (attempt + 1)); continue
        try: return dt, r.status_code, r.json()
        except Exception: return dt, r.status_code, {"text": r.text[:300]}
    return dt, r.status_code, {}

done = 0
for p in pairs:
    if done >= 120: break
    lines = norm_read(p["content"])
    n = len(lines)
    if n < 30 or n > 4000: continue
    old_first = p["old"].split("\n")[0].strip()
    if not old_first:
        continue
    hit = [i for i, l in enumerate(lines) if l.strip() == old_first]
    if not hit: 
        hit = [i for i, l in enumerate(lines) if old_first and old_first in l]
    if len(hit) != 1: continue
    a = hit[0]; b = a + max(0, p["old"].count("\n"))
    k = max(8, math.ceil(n / 200))
    nw = math.ceil(n / k)
    if nw > 255: continue
    state_lines = []
    for i, l in enumerate(lines):
        state_lines.append(f"W{i//k+1:03d}| {l[:300]}")
    state = {"worker_task": p["task"], "worker_intent_before_read": p["intent"], "file_path": p["path"],
             "file_lines_prefixed_by_window_id": "\n".join(state_lines)}
    qs = {}
    for w in range(1, nw + 1):
        qs[f"W{w:03d}"] = {"type": "score", "instructions": f"Does the worker need window W{w:03d} of this file to carry out its intent?",
                           "criteria": ["not needed", "useful context", "must read: the worker will edit or depend on these lines"]}
    qs["edit_window"] = {"type": "choice", "instructions": "Which window of the file will the worker edit first to carry out its intent?", "criteria": {f"W{w:03d}": None for w in range(1, nw + 1)}}
    dt, st, body = call(state, qs)
    chars_total = sum(len(l) for l in lines)
    raw.append({"status": st, "ms": round(dt*1000), "usage": body.get("usage"), "n_lines": n, "windows": nw, "chars": chars_total})
    if st != 200: print("status", st, json.dumps(body)[:200]); continue
    hit_windows = {f"W{i//k+1:03d}" for i in range(a, b + 1)}
    row = {"file": p["file"].split("/")[-1], "path": p["path"], "n_lines": n, "chars": chars_total, "windows": nw, "k": k, "edit_lines": [a+1, b+1], "hit_windows": sorted(hit_windows)}
    for thr in (0.5, 1.0, 1.5):
        kept = {w for w in qs if w != "edit_window" and body["answers"][w]["score"] >= thr}
        kept_chars = sum(len(l) for i, l in enumerate(lines) if f"W{i//k+1:03d}" in kept)
        row[f"keep@{thr}"] = {"windows": len(kept), "chars_share": round(kept_chars / max(1, chars_total), 3), "covers_edit": hit_windows <= kept}
    ch = body["answers"]["edit_window"]; top = sorted(ch["probabilities"].items(), key=lambda x: -x[1])
    row["choice"] = ch["choice"]; row["choice_conf"] = ch["confidence"]
    row["choice_top1"] = ch["choice"] in hit_windows
    row["choice_top3"] = any(w in hit_windows for w, _ in top[:3])
    # a 'top-3 windows plus neighbours' policy: keep the 3 most likely edit windows and one neighbour each side
    sel = set()
    for w, _ in top[:3]:
        idx = int(w[1:]); sel |= {f"W{x:03d}" for x in (idx-1, idx, idx+1) if 1 <= x <= nw}
    sel_chars = sum(len(l) for i, l in enumerate(lines) if f"W{i//k+1:03d}" in sel)
    row["top3nbr"] = {"chars_share": round(sel_chars / max(1, chars_total), 3), "covers_edit": hit_windows <= sel}
    rows.append(row); done += 1
json.dump({"rows": rows, "raw": raw}, open(f"{OUT}/readfor.json", "w"), indent=1)

N = len(rows)
print(f"\nscored pairs {N}; file lines median {statistics.median(r['n_lines'] for r in rows):.0f}; chars median {statistics.median(r['chars'] for r in rows):.0f}; windows median {statistics.median(r['windows'] for r in rows):.0f}")
for pol in ("keep@0.5", "keep@1.0", "keep@1.5", "top3nbr"):
    cov = sum(r[pol]["covers_edit"] for r in rows) / N
    share = statistics.mean(r[pol]["chars_share"] for r in rows)
    wshare = sum(r[pol]["chars_share"] * r["chars"] for r in rows) / sum(r["chars"] for r in rows)
    print(f"{pol:9s} covers edited lines {cov:6.1%}   mean chars kept {share:6.1%}   byte-weighted kept {wshare:6.1%}")
print(f"edit window top-1 {sum(r['choice_top1'] for r in rows)/N:.1%}, top-3 {sum(r['choice_top3'] for r in rows)/N:.1%}")
for thr in (0.8, 0.9, 0.95):
    sub = [r for r in rows if r["choice_conf"] >= thr]
    if sub: print(f"  choice conf>={thr}: coverage {len(sub)/N:.1%}, top-1 acc {sum(r['choice_top1'] for r in sub)/len(sub):.1%}")
ok = [r for r in raw if r["status"] == 200]
print(f"calls {len(raw)} ok {len(ok)}; latency ms median {statistics.median(r['ms'] for r in ok):.0f} p90 {sorted(r['ms'] for r in ok)[int(len(ok)*0.9)]} max {max(r['ms'] for r in ok)}; input tokens total {sum(r['usage']['input_tokens'] for r in ok)} mean {statistics.mean(r['usage']['input_tokens'] for r in ok):.0f}; cost ${sum(r['usage']['input_tokens'] for r in ok)/1e6*0.042:.4f}")
