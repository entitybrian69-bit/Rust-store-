#!/usr/bin/env python3
"""Verification for Biomes Unseen: registry completeness vs assets/lang/loot/recipes."""
import json, sys, zipfile
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent
M = ROOT/"src-mods/biomes_unseen"; ID="biomes_unseen"
fails=[]
def check(c,msg):
    if not c: fails.append(msg)
lang=json.loads((M/f"assets/{ID}/lang/en_us.json").read_text())
blocks=sorted(p.stem for p in (M/f"data/{ID}/horizon/block").glob("*.json"))
items=sorted(p.stem for p in (M/f"data/{ID}/horizon/item").glob("*.json"))
for b in blocks:
    for rel in [f"assets/{ID}/blockstates/{b}.json",f"assets/{ID}/textures/block/{b}.png",f"data/{ID}/loot_table/blocks/{b}.json"] + ([] if b.endswith("_wall_sign") else [f"assets/{ID}/models/item/{b}.json"]):
        check((M/rel).exists(), f"missing {rel}")
    check(f"block.{ID}.{b}" in lang, f"missing lang block {b}")
    loot=json.loads((M/f"data/{ID}/loot_table/blocks/{b}.json").read_text())
    check(loot["pools"][0]["entries"][0]["name"]==f"{ID}:{b}", f"loot mismatch {b}")
for i in items:
    for rel in [f"assets/{ID}/models/item/{i}.json",f"assets/{ID}/textures/item/{i}.png"]:
        check((M/rel).exists(), f"missing {rel}")
    check(f"item.{ID}.{i}" in lang, f"missing lang item {i}")
# every texture referenced by a model must exist
for mp in (M/f"assets/{ID}/models").rglob("*.json"):
    for ref in json.loads(mp.read_text()).get("textures",{}).values():
        ns,path=ref.split(":"); check((M/f"assets/{ns}/textures/{path}.png").exists(), f"{mp.name} -> missing texture {ref}")
# recipes must reference registered or vanilla ids
known={f"{ID}:{x}" for x in blocks+items}
for rp in (M/f"data/{ID}/recipe").glob("*.json"):
    r=json.loads(rp.read_text()); res=r["result"]["id"]; check(res in known, f"recipe {rp.name} result {res} unregistered")
    refs=list(r.get("key",{}).values())+r.get("ingredients",[])
    for ref in refs: check(isinstance(ref,str) and (ref in known or ref.startswith("minecraft:")), f"recipe {rp.name} bad ref {ref}")
# worldgen: biomes, configured/placed features cross-reference
wg=M/f"data/{ID}/worldgen"
biomes=sorted(x.stem for x in (wg/"biome").glob("*.json"))
check(len(biomes)==44, f"biome count {len(biomes)}")
placed={x.stem for x in (wg/"placed_feature").glob("*.json")}
conf={x.stem for x in (wg/"feature").glob("*.json")}
check(placed==conf, "placed != configured feature sets")
for b in biomes:
    d=json.loads((wg/f"biome/{b}.json").read_text())
    for k in ("temperature","downfall","has_precipitation","features","horizon:climate"): check(k in d, f"biome {b} missing {k}")
    c=d["horizon:climate"]
    for k in ("temperature","humidity","continentalness","erosion","depth","weirdness"):
        v=c[k]; check(isinstance(v,list) and len(v)==2 and -2<=v[0]<=v[1]<=2, f"biome {b} climate {k}={v}")
    for step in d["features"]:
        for f in step:
            check(f.startswith(f"{ID}:") and f.split(":")[1] in placed, f"biome {b} feature {f} not placed")
for f in conf:
    d=json.loads((wg/f"feature/{f}.json").read_text())
    txt=json.dumps(d)
    import re as _re
    for ref in _re.findall(r'"Name"\s*:\s*"([^"]+)"', txt):
        if ref.startswith(f"{ID}:"): check(ref.split(":")[1] in blocks, f"feature {f} block {ref} unregistered")
# variant blockstate coverage (vanilla state spaces)
for w in ["fir","redwood"]:
    st=json.loads((M/f"assets/{ID}/blockstates/{w}_stairs.json").read_text())["variants"]
    check(len(st)==40, f"{w}_stairs variants {len(st)}")
    sl=json.loads((M/f"assets/{ID}/blockstates/{w}_slab.json").read_text())["variants"]
    check(len(sl)==3, f"{w}_slab variants")
    dr=json.loads((M/f"assets/{ID}/blockstates/{w}_door.json").read_text())["variants"]
    check(len(dr)==64, f"{w}_door variants {len(dr)}")
# creative tab covers everything
tab=json.loads((M/f"data/{ID}/horizon/creative_tab/biomes_unseen_tab.json").read_text())
check(set(tab["items"])==known, "creative tab != registry")
# PNG headers + hnmod
for p in M.rglob("*.png"): check(p.read_bytes()[:8]==b"\x89PNG\r\n\x1a\n", f"bad png {p}")
for name in ["releases/biomes_unseen.hmod","releases/biomes_unseen.hnmod"]:
    check((ROOT/name).exists(), f"missing {name}")
with zipfile.ZipFile(ROOT/"releases/biomes_unseen.hnmod") as zf:
    names=set(zf.namelist())
    check("mixins/biomes_unseen/horizon-mixin.toml" in names, "hnmod missing horizon-mixin.toml")
    man=json.loads(zf.read("horizon-native.json"))
    check(set(man)<={"schema","id","version","game","horizon","mixins","hooks"}, f"hnmod unknown fields {set(man)}")
    check(man["schema"]==1 and man["id"]==ID and man["game"]=="26.3", "hnmod manifest fields")
    import re as _r
    check(bool(_r.fullmatch(r"[0-9a-f]{40}", man["horizon"])) and bool(_r.fullmatch(r"[0-9a-f]{40}", man["mixins"])), "hnmod revisions not 40-hex")
    check(all(n=="horizon-native.json" or n.startswith(f"mixins/{ID}/") for n in names), "hnmod stray entries")
print(f"blocks={len(blocks)} items={len(items)} lang={len(lang)} FAILURES={len(fails)}")
for f in fails: print("FAIL:",f)
sys.exit(1 if fails else 0)
