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
    for rel in [f"assets/{ID}/blockstates/{b}.json",f"assets/{ID}/models/block/{b}.json",f"assets/{ID}/models/item/{b}.json",f"assets/{ID}/textures/block/{b}.png",f"data/{ID}/loot_table/blocks/{b}.json"]:
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
# creative tab covers everything
tab=json.loads((M/f"data/{ID}/horizon/creative_tab/biomes_unseen_tab.json").read_text())
check(set(tab["items"])==known, "creative tab != registry")
# PNG headers + hnmod
for p in M.rglob("*.png"): check(p.read_bytes()[:8]==b"\x89PNG\r\n\x1a\n", f"bad png {p}")
for name in ["releases/biomes_unseen.hmod","releases/biomes_unseen.hnmod"]:
    check((ROOT/name).exists(), f"missing {name}")
with zipfile.ZipFile(ROOT/"releases/biomes_unseen.hnmod") as zf:
    names=set(zf.namelist()); check("horizon-native.json" in names and "mixins/biomes_unseen/src/lib.rs" in names, "hnmod contents")
print(f"blocks={len(blocks)} items={len(items)} lang={len(lang)} FAILURES={len(fails)}")
for f in fails: print("FAIL:",f)
sys.exit(1 if fails else 0)
