import os
import json, re, time, glob, statistics, sys, requests
OUT = os.environ.get("REPLAY_OUT", ".")
key = os.environ.get("TYPESAFE_API_KEY")  # export it first; never commit it
assert key, "no key"
URL = "https://api.typesafe.ai/v1/systemone"
H = {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}
raw = []
def call(tag, state, questions, model="jev-latest"):
    t = time.perf_counter()
    r = requests.post(URL, headers=H, json={"state": state, "model": model, "questions": questions}, timeout=60)
    dt = time.perf_counter() - t
    try: body = r.json()
    except Exception: body = {"text": r.text[:300]}
    raw.append({"tag": tag, "status": r.status_code, "ms": round(dt*1000), "body": body, "req_id": r.headers.get("x-typesafe-request-id", "-")})
    return dt, r.status_code, body

# --- Test 1: latency, small public state
state1 = "ripgrep is a line-oriented search tool that recursively searches the current directory for a regex pattern. By default, ripgrep respects gitignore rules and skips hidden files, directories and binary files."
q1 = {"cli": {"type": "noul", "instructions": "Is this text describing a command-line tool?"},
      "kind": {"type": "choice", "instructions": "What kind of software is described?", "criteria": {"search tool": None, "database": None, "video game": None}},
      "tech": {"type": "score", "instructions": "How technical is this text?", "criteria": ["not technical", "somewhat technical", "very technical"]}}
lat = []
for i in range(7):
    dt, st, body = call(f"lat{i}", state1, q1)
    lat.append(dt*1000)
    if i == 0:
        print("T1 status", st, "model", body.get("model"), "usage", body.get("usage"))
        print("T1 answers", json.dumps(body.get("answers"))[:400])
print(f"T1 latency ms over 7 calls: min {min(lat):.0f} median {statistics.median(lat):.0f} max {max(lat):.0f}")

# --- Test 2: line selection on a public stdlib file
f = sorted(glob.glob("/usr/lib/python3*/json/decoder.py"))[-1]
lines = open(f).read().splitlines()[:255]
state2 = "\n".join(f"L{i+1:03d}| {l}" for i, l in enumerate(lines))
ids = {f"L{i+1:03d}": None for i in range(len(lines))}
q2 = {"cls": {"type": "choice", "instructions": "Which line begins the definition of the class JSONDecodeError?", "criteria": ids},
      "scan": {"type": "choice", "instructions": "Which line begins the definition of the function py_scanstring?", "criteria": ids},
      "has_cls": {"type": "noul", "instructions": "Does any line define a class named JSONDecodeError?"},
      "has_yaml": {"type": "noul", "instructions": "Does any line define a class named YAMLParser?"}}
dt, st, body = call("lines", state2, q2)
print(f"\nT2 {f} lines={len(lines)} chars={len(state2)} status={st} ms={dt*1000:.0f} usage={body.get('usage')}")
truth = {"cls": next(f"L{i+1:03d}" for i,l in enumerate(lines) if l.startswith("class JSONDecodeError")),
         "scan": next(f"L{i+1:03d}" for i,l in enumerate(lines) if l.startswith("def py_scanstring"))}
for k in ("cls", "scan"):
    a = body["answers"][k]; top = sorted(a["probabilities"].items(), key=lambda x: -x[1])[:3]
    print(f"T2 {k}: choice={a['choice']} truth={truth[k]} ok={a['choice']==truth[k]} conf={a['confidence']:.2f} top3={[(t, round(p,2)) for t,p in top]}")
print("T2 has_cls", body["answers"]["has_cls"]["noul"], "has_yaml", body["answers"]["has_yaml"]["noul"])

# --- Test 3: pack trimming on factory's own PRD-001 (owner's doc, not client code)
prd = open("/home/emiliano/Projects/factory/docs/features/001-driver/prd.md").read()
parts = re.split(r'(?m)^(?=## )', prd)
secs = []
for p in parts:
    m = re.match(r'^## (.+)$', p, re.M)
    if not m: continue
    secs.append({"id": f"S{len(secs)+1:02d}", "heading": m.group(1).strip(), "text": p[len(m.group(0)):].strip()[:1500]})
slice_title = "FR-10 stub harness: a fixture repo plus stub `claude` and `gh` binaries so `cargo test` can drive a whole fake run offline"
q3 = {s["id"]: {"type": "score", "instructions": f"A worker is about to build this slice: \"{slice_title}\". How necessary is section {s['id']} (\"{s['heading']}\") in its context pack?",
                 "criteria": ["not needed for this slice", "useful background", "needed to build the slice"]} for s in secs}
q3["first"] = {"type": "choice", "instructions": f"Which single section must the worker read first to build the slice \"{slice_title}\"?", "criteria": {s["id"]: s["heading"] for s in secs}}
dt, st, body = call("pack", secs, q3)
print(f"\nT3 PRD sections={len(secs)} state_chars={sum(len(s['text']) for s in secs)} status={st} ms={dt*1000:.0f} usage={body.get('usage')}")
if st == 200:
    for s in secs:
        a = body["answers"][s["id"]]
        print(f"T3 {s['id']} score={a['score']:.2f} conf={a['confidence']:.2f}  {s['heading'][:60]}")
    a = body["answers"]["first"]; top = sorted(a["probabilities"].items(), key=lambda x: -x[1])[:3]
    print("T3 first:", a["choice"], f"conf={a['confidence']:.2f}", [(t, round(p,2)) for t,p in top])
else:
    print("T3 body", json.dumps(body)[:600])
json.dump(raw, open(f"{OUT}/raw.json", "w"), indent=1)
