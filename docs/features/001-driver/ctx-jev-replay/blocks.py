# Replay: can Jev predict the owner's answer to a block (AskUserQuestion)? Ground truth = the owner's real pick.
import os
import json, re, glob, time, statistics, requests
OUT = os.environ.get("REPLAY_OUT", ".")
key = os.environ.get("TYPESAFE_API_KEY")  # export it first; never commit it
URL = "https://api.typesafe.ai/v1/systemone"; H = {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}
roots = ["/home/emiliano/.claude/projects/-home-emiliano-Projects-ICG-icg-cloud", "/home/emiliano/.claude/projects/-home-emiliano-Projects-Prokey-arca-api"]
files = []
for r in roots: files += glob.glob(f"{r}/*.jsonl") + glob.glob(f"{r}/*/subagents/agent-*.jsonl")

def text_of(content):
    if isinstance(content, str): return content
    return "\n".join(c.get("text", "") for c in content if isinstance(c, dict) and c.get("type") == "text")

cases = []  # one per AskUserQuestion use
for f in files:
    pending = {}; recent_assistant = []; last_user = ""
    for line in open(f):
        try: j = json.loads(line)
        except: continue
        msg = j.get("message") or {}; content = msg.get("content"); role = msg.get("role")
        if role == "user" and isinstance(content, str): last_user = content
        if not isinstance(content, list): continue
        if role == "user" and content and content[0].get("type") == "text": last_user = text_of(content)
        if role == "assistant":
            t = text_of(content)
            if t.strip(): recent_assistant.append(t)
        for c in content:
            if c.get("type") == "tool_use" and c.get("name") == "AskUserQuestion":
                ctx = "\n\n".join(recent_assistant[-3:])[-6000:]
                pending[c["id"]] = {"file": f, "input": c["input"], "context": ctx, "last_user": last_user[-1500:]}
            elif c.get("type") == "tool_result" and c.get("tool_use_id") in pending:
                case = pending.pop(c["tool_use_id"]); case["result"] = text_of(c.get("content")) if not isinstance(c.get("content"), str) else c["content"]
                cases.append(case)

def parse_answers(result, questions):
    out = {}
    qs = [q["question"] for q in questions]
    for q in qs:
        k = f'"{q}"="'; i = result.find(k)
        if i < 0: out[q] = None; continue
        start = i + len(k)
        ends = [result.find(f'"{q2}"="', start) for q2 in qs if q2 != q]
        ends = [e - 3 for e in ends if e >= 0]
        e2 = result.find('". You can now', start)
        if e2 >= 0: ends.append(e2)
        end = min(ends) if ends else len(result)
        out[q] = result[start:end]
    return out

def strip_rec(s): return re.sub(r'\s*\(Recommended\)', '', s or "")

raw = []; rows = []; lat = []
def call(state, questions):
    for attempt in range(4):
        t = time.perf_counter()
        r = requests.post(URL, headers=H, json={"state": state, "model": "jev-latest", "questions": questions}, timeout=60)
        dt = time.perf_counter() - t
        if r.status_code in (429, 529): time.sleep(1.5 * (attempt + 1)); continue
        return dt, r.status_code, r.json()
    return dt, r.status_code, {}

for ci, case in enumerate(cases):
    qs = case["input"].get("questions", [])
    answers = parse_answers(case.get("result", ""), qs)
    for variant in ("asis", "stripped"):
        qmap = {}; meta = {}
        for qi, q in enumerate(qs):
            opts = q.get("options", [])
            if len(opts) < 2: continue
            crit = {}; labels = []
            for oi, o in enumerate(opts):
                lab = o["label"] if variant == "asis" else strip_rec(o["label"])
                if lab in crit: lab = f"{lab} [{oi}]"
                crit[lab] = (o.get("description") if variant == "asis" else strip_rec(o.get("description"))) or None
                labels.append((lab, o["label"]))
            qmap[f"q{qi}"] = {"type": "choice", "instructions": f"Given the owner's context, which option will the project owner choose for: {q['question']}", "criteria": crit}
            meta[f"q{qi}"] = (q, labels)
        if not qmap: continue
        state = {"recent_assistant_text": case["context"], "owner_last_message": case["last_user"],
                 "questions": [{"question": q["question"], "options": [{"label": (o["label"] if variant == "asis" else strip_rec(o["label"])), "description": (o.get("description") if variant == "asis" else strip_rec(o.get("description")))} for o in q.get("options", [])]} for q in qs]}
        dt, st, body = call(state, qmap)
        lat.append(dt * 1000); raw.append({"case": ci, "variant": variant, "status": st, "ms": round(dt * 1000), "usage": body.get("usage")})
        if st != 200: continue
        for qk, (q, labels) in meta.items():
            a = body["answers"].get(qk); truth = answers.get(q["question"])
            orig_by_lab = dict(labels)
            truth_lab = None
            if truth is not None:
                for lab, orig in labels:
                    if truth.strip() == orig.strip(): truth_lab = lab
                if truth_lab is None and q.get("multiSelect"):
                    parts = [p.strip() for p in truth.split(", ")]
                    for lab, orig in labels:
                        if orig.strip() in parts: truth_lab = lab; break
            rec = next((lab for lab, orig in labels if "(Recommended)" in orig), None)
            rows.append({"case": ci, "variant": variant, "file": case["file"].split("/")[-1], "question": q["question"][:120], "n_options": len(labels),
                         "truth": truth_lab, "truth_raw": (truth or "")[:80], "recommended": rec, "choice": a["choice"], "confidence": a["confidence"],
                         "p_choice": a["probabilities"].get(a["choice"]), "multi": bool(q.get("multiSelect")), "tokens": body.get("usage", {}).get("input_tokens")})
json.dump({"rows": rows, "raw": raw}, open(f"{OUT}/blocks.json", "w"), indent=1)

# --- summary
def summarize(variant):
    R = [r for r in rows if r["variant"] == variant and r["truth"] is not None]
    n = len(R); acc = sum(r["choice"] == r["truth"] for r in R) / n
    rec_rows = [r for r in R if r["recommended"]]
    base = sum(r["recommended"] == r["truth"] for r in rec_rows) / len(rec_rows) if rec_rows else float("nan")
    print(f"\n[{variant}] questions with matched truth: {n} (of {sum(1 for r in rows if r['variant']==variant)}); Jev accuracy {acc:.1%}; 'pick Recommended' baseline {base:.1%} on {len(rec_rows)} with a recommendation")
    dev = [r for r in rec_rows if r["recommended"] != r["truth"]]
    print(f"  owner deviated from Recommended in {len(dev)}; Jev caught {sum(r['choice']==r['truth'] for r in dev)} of those")
    print("  thr  coverage  accuracy   n")
    for thr in (0.5, 0.7, 0.8, 0.9, 0.95, 0.99):
        sub = [r for r in R if r["confidence"] >= thr]
        if sub: print(f"  {thr:<4} {len(sub)/n:8.1%} {sum(r['choice']==r['truth'] for r in sub)/len(sub):9.1%} {len(sub):4d}")
        else: print(f"  {thr:<4}     0.0%       n/a    0")
summarize("asis"); summarize("stripped")
unm = [r for r in rows if r["variant"] == "asis" and r["truth"] is None]
print(f"\nunmatched (free-text 'Other' or parse miss): {len(unm)}; sample: {[r['truth_raw'][:50] for r in unm[:5]]}")
toks = [r["usage"]["input_tokens"] for r in raw if r.get("usage")]
print(f"calls {len(raw)}; latency ms median {statistics.median(lat):.0f} p90 {sorted(lat)[int(len(lat)*0.9)]:.0f} max {max(lat):.0f}; input tokens total {sum(toks)} mean {statistics.mean(toks):.0f}; cost ${sum(toks)/1e6*0.042:.4f}")
