#!/usr/bin/env python3
"""Generate Biomes Unseen (biomes_unseen) Horizon API 0.2 content mod.

Original procedural textures only (no Biomes O' Plenty assets copied).
Uses ONLY registry types proven by existing mods in this repo:
horizon/block, horizon/item, horizon/creative_tab, recipe, loot_table, tags.
Biome/worldgen registration is NOT in API 0.2 (see docs/GAPS.md in mod docs);
biome climate data ships as Rust source in the .hnmod bundle.
"""
import json, struct, zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MOD = ROOT / "src-mods" / "biomes_unseen"
ID = "biomes_unseen"

def png(pixels):
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t+d) & 0xffffffff)
    raw = bytearray()
    for y in range(16):
        raw.append(0)
        for x in range(16):
            raw.extend(pixels[y*16+x])
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB",16,16,8,6,0,0,0))
            + chunk(b"IDAT", zlib.compress(bytes(raw),9)) + chunk(b"IEND", b""))

def tex(base, border, accent, pattern="bevel"):
    px=[]
    for y in range(16):
        for x in range(16):
            if x in (0,15) or y in (0,15): c=border
            elif pattern=="ore" and ((x in (3,4,10,11) and y in (4,5,11)) or (x in (6,7,12) and y in (7,8,12))): c=accent
            elif pattern=="leaves" and (x*7+y*13)%5==0: c=accent
            elif pattern=="grain" and x%4==0: c=accent
            elif pattern=="cross" and (x==y or x==15-y): c=accent
            elif (x+y)%4==0: c=tuple(min(255,v+10) for v in base)
            else: c=base
            px.append((c[0],c[1],c[2],255))
    return png(px)

def item_tex(fg, edge, shape="gem"):
    px=[]
    for y in range(16):
        for x in range(16):
            dx,dy=abs(x-7.5),abs(y-7.5)
            if shape=="orb":
                d=dx*dx+dy*dy
                px.append((fg[0],fg[1],fg[2],255) if d<=18 else (edge[0],edge[1],edge[2],255) if d<=28 else (0,0,0,0))
            elif shape=="sword":
                diff=abs((15-y)-x)
                if x<5 and y>10 and diff<=2: px.append((110,75,42,255))
                elif diff==0 and 2<=x<=13: px.append((fg[0],fg[1],fg[2],255))
                elif diff==1 and 3<=x<=13 and y<=12: px.append((edge[0],edge[1],edge[2],255))
                else: px.append((0,0,0,0))
            else:
                m=dx+dy
                px.append((fg[0],fg[1],fg[2],255) if m<=4.5 else (edge[0],edge[1],edge[2],255) if m<=6 else (0,0,0,0))
    return png(px)

def wj(p, data): p.parent.mkdir(parents=True, exist_ok=True); p.write_text(json.dumps(data, indent=2)+"\n")
def wb(p, data): p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(data)

LANG={}
BLOCKS=[]  # (name, display, horizon_block_dict, model_kind, palette)
ITEMS=[]   # (name, display, horizon_item_dict, palette, shape)

def block(name, display, template, props, kind, palette, sounds=None, occludes=None):
    d={"template":template,"properties":{**props, **({"sounds":sounds} if sounds else {})},"item":True}
    if occludes is not None: d["occludes"]=occludes
    BLOCKS.append((name, display, d, kind, palette)); LANG[f"block.{ID}.{name}"]=display

def item(name, display, template, comps, palette, shape="gem"):
    ITEMS.append((name, display, {"template":template,"components":comps}, palette, shape)); LANG[f"item.{ID}.{name}"]=display

# ---- Wood sets (Biomes O' Plenty parity names) ----
WOODS = {
 "cherry":((196,120,130),(150,80,90),(230,170,180)), "dead":((120,110,100),(85,78,70),(90,85,80)),
 "fir":((105,82,55),(74,56,36),(45,80,40)), "hellbark":((90,45,40),(60,28,25),(70,30,30)),
 "jacaranda":((150,110,165),(105,75,120),(140,100,180)), "magic":((80,90,160),(52,60,115),(90,110,220)),
 "mahogany":((130,70,50),(92,48,34),(60,95,45)), "maple":((165,105,60),(120,72,38),(190,110,40)),
 "palm":((190,160,95),(140,115,65),(70,140,60)), "redwood":((150,85,60),(105,58,40),(55,90,45)),
 "umbran":((70,60,90),(48,40,62),(50,45,70)), "willow":((130,125,85),(95,90,60),(95,135,80)),
}
for w,(base,border,leaf) in WOODS.items():
    disp=w.replace("_"," ").title()
    block(f"{w}_log", f"{disp} Log", "minecraft:oak_log", {"hardness":2.0,"resistance":2.0}, "log", (base,border,leaf), "minecraft:wood")
    block(f"{w}_planks", f"{disp} Planks", "minecraft:oak_planks", {"hardness":2.0,"resistance":3.0}, "cube", (base,border,leaf), "minecraft:wood")
    block(f"{w}_leaves", f"{disp} Leaves", "minecraft:oak_leaves", {"hardness":0.2,"resistance":0.2}, "leaves", (leaf,border,base), "minecraft:grass")

# ---- Plants & flowers ----
PLANTS = {
 "lavender":((150,120,200),"minecraft:allium"), "orange_cosmos":((240,150,50),"minecraft:poppy"),
 "pink_daffodil":((245,170,200),"minecraft:pink_tulip"), "pink_hibiscus":((240,110,160),"minecraft:peony"),
 "white_anemone":((240,240,235),"minecraft:white_tulip"), "wildflower":((240,210,80),"minecraft:dandelion"),
 "violet":((130,90,200),"minecraft:cornflower"), "burning_blossom":((230,90,40),"minecraft:torchflower"),
 "glowshroom":((120,220,200),"minecraft:brown_mushroom"), "waterlily_pad":((60,140,70),"minecraft:lily_pad"),
 "cattail":((150,110,60),"minecraft:tall_grass"), "rose":((200,40,60),"minecraft:rose_bush"),
}
for n,(col,tpl) in PLANTS.items():
    block(n, n.replace("_"," ").title(), tpl, {"hardness":0.0,"resistance":0.0}, "cross", (col,(90,120,60),(255,255,255)), "minecraft:grass")

# ---- Terrain / nether / misc blocks ----
block("ash_block","Ash Block","minecraft:sand",{"hardness":0.5,"resistance":0.5},"cube",((170,170,170),(130,130,130),(200,200,200)),"minecraft:sand")
block("brimstone","Brimstone","minecraft:netherrack",{"hardness":0.4,"resistance":0.4},"cube",((180,160,60),(130,115,40),(220,200,90)),"minecraft:netherrack")
block("flesh_block","Flesh Block","minecraft:nether_wart_block",{"hardness":0.4,"resistance":0.4},"cube",((170,80,90),(125,55,65),(200,110,120)),"minecraft:nether_wart_block")
block("porous_flesh","Porous Flesh","minecraft:nether_wart_block",{"hardness":0.4,"resistance":0.4},"ore",((160,75,85),(115,50,60),(220,150,160)),"minecraft:nether_wart_block")
block("origin_grass_block","Origin Grass Block","minecraft:grass_block",{"hardness":0.6,"resistance":0.6},"cube",((110,170,80),(90,65,40),(140,200,100)),"minecraft:grass")
block("tanzanite_ore","Tanzanite Ore","minecraft:emerald_ore",{"hardness":3.0,"resistance":3.0},"ore",((128,128,128),(90,90,90),(110,90,220)),"minecraft:stone")
block("tanzanite_block","Block of Tanzanite","minecraft:emerald_block",{"hardness":5.0,"resistance":6.0},"cube",((110,90,220),(75,60,160),(150,130,255)),"minecraft:metal")
block("mud_brick_block","Mud Brick Block","minecraft:mud_bricks",{"hardness":1.5,"resistance":3.0},"cube",((130,105,80),(95,75,55),(160,135,105)),"minecraft:mud_bricks")

# ---- Items ----
item("tanzanite","Tanzanite","minecraft:emerald",{"minecraft:max_stack_size":64,"minecraft:rarity":"uncommon"},((110,90,220),(75,60,160)))
item("tanzanite_sword","Tanzanite Sword","minecraft:diamond_sword",{"minecraft:max_damage":1700,"minecraft:rarity":"rare","minecraft:attribute_modifiers":[{"type":"minecraft:attack_damage","id":"minecraft:base_attack_damage","amount":7.5,"operation":"add_value","slot":"mainhand"}]},((110,90,220),(75,60,160)),"sword")
item("tanzanite_pickaxe","Tanzanite Pickaxe","minecraft:diamond_pickaxe",{"minecraft:max_damage":1700,"minecraft:rarity":"rare"},((110,90,220),(75,60,160)),"sword")
item("mudball","Mudball","minecraft:snowball",{"minecraft:max_stack_size":16},((130,105,80),(95,75,55)),"orb")
item("origin_acorn","Origin Acorn","minecraft:oak_sapling",{"minecraft:max_stack_size":64},((140,110,60),(95,75,40)),"orb")
for w in WOODS: item(f"{w}_sapling", f"{w.replace('_',' ').title()} Sapling", "minecraft:oak_sapling", {"minecraft:max_stack_size":64}, ((90,140,70),(60,95,45)), "orb")

def main():
    MOD.mkdir(parents=True, exist_ok=True)
    (MOD/"horizon-mod.toml").write_text(
      'id = "biomes_unseen"\nname = "Biomes Unseen"\nversion = "1.0.0"\n'
      'description = "Biomes O\' Plenty-style expansion for Horizon 26.3: 12 wood sets, flowers, plants, nether flesh/brimstone, tanzanite gem tier and more. Biome registration is not exposed by Horizon API 0.2 - see docs/GAPS.md."\n'
      'authors = ["Biomes Unseen Contributors"]\napi = "0.2"\nside = "both"\ncapabilities = ["registries"]\n')
    axe, pick = [], []
    for name,disp,d,kind,pal in BLOCKS:
        wj(MOD/f"data/{ID}/horizon/block/{name}.json", d)
        base,border,accent = pal
        wb(MOD/f"assets/{ID}/textures/block/{name}.png", tex(base,border,accent, "leaves" if kind=="leaves" else "ore" if kind=="ore" else "grain" if kind=="log" else "bevel"))
        if kind=="log":
            wb(MOD/f"assets/{ID}/textures/block/{name}_top.png", tex(border,base,accent,"cross"))
            model={"parent":"minecraft:block/cube_column","textures":{"end":f"{ID}:block/{name}_top","side":f"{ID}:block/{name}"}}
        elif kind=="cross":
            model={"parent":"minecraft:block/cross","textures":{"cross":f"{ID}:block/{name}"}}
        else:
            model={"parent":"minecraft:block/cube_all","textures":{"all":f"{ID}:block/{name}"}}
        wj(MOD/f"assets/{ID}/models/block/{name}.json", model)
        wj(MOD/f"assets/{ID}/blockstates/{name}.json", {"variants":{"":{"model":f"{ID}:block/{name}"}}})
        wj(MOD/f"assets/{ID}/models/item/{name}.json", {"parent":f"{ID}:block/{name}"})
        wj(MOD/f"data/{ID}/loot_table/blocks/{name}.json", {"type":"minecraft:block","pools":[{"rolls":1.0,"bonus_rolls":0.0,"entries":[{"type":"minecraft:item","name":f"{ID}:{name}"}],"conditions":[{"condition":"minecraft:survives_explosion"}]}]})
        (axe if name.endswith(("_log","_planks")) else pick if name in ("tanzanite_ore","tanzanite_block","brimstone") else axe if kind=="leaves" else pick).append(f"{ID}:{name}")
    for name,disp,d,pal,shape in ITEMS:
        wj(MOD/f"data/{ID}/horizon/item/{name}.json", d)
        wb(MOD/f"assets/{ID}/textures/item/{name}.png", item_tex(pal[0],pal[1],shape))
        wj(MOD/f"assets/{ID}/models/item/{name}.json", {"parent":"minecraft:item/handheld" if shape=="sword" else "minecraft:item/generated","textures":{"layer0":f"{ID}:item/{name}"}})

    # Recipes: planks from each log, tanzanite storage, tools, mud bricks
    for w in WOODS:
        wj(MOD/f"data/{ID}/recipe/{w}_planks.json", {"type":"minecraft:crafting_shapeless","ingredients":[f"{ID}:{w}_log"],"result":{"id":f"{ID}:{w}_planks","count":4}})
    wj(MOD/f"data/{ID}/recipe/tanzanite_block.json", {"type":"minecraft:crafting_shaped","pattern":["TTT","TTT","TTT"],"key":{"T":f"{ID}:tanzanite"},"result":{"id":f"{ID}:tanzanite_block","count":1}})
    wj(MOD/f"data/{ID}/recipe/tanzanite_sword.json", {"type":"minecraft:crafting_shaped","pattern":[" T "," T "," S "],"key":{"T":f"{ID}:tanzanite","S":"minecraft:stick"},"result":{"id":f"{ID}:tanzanite_sword","count":1}})
    wj(MOD/f"data/{ID}/recipe/tanzanite_pickaxe.json", {"type":"minecraft:crafting_shaped","pattern":["TTT"," S "," S "],"key":{"T":f"{ID}:tanzanite","S":"minecraft:stick"},"result":{"id":f"{ID}:tanzanite_pickaxe","count":1}})
    wj(MOD/f"data/{ID}/recipe/mud_brick_block.json", {"type":"minecraft:crafting_shaped","pattern":["MM","MM"],"key":{"M":f"{ID}:mudball"},"result":{"id":f"{ID}:mud_brick_block","count":1}})

    wj(MOD/"data/minecraft/tags/block/mineable/axe.json", {"replace":False,"values":sorted(set(axe))})
    wj(MOD/"data/minecraft/tags/block/mineable/pickaxe.json", {"replace":False,"values":sorted(set(pick))})
    all_ids=[f"{ID}:{n}" for n, *_ in BLOCKS]+[f"{ID}:{n}" for n, *_ in ITEMS]
    wj(MOD/f"data/{ID}/horizon/creative_tab/biomes_unseen_tab.json", {"title":"Biomes Unseen","icon":f"{ID}:fir_log","items":all_ids})
    LANG[f"itemGroup.{ID}.biomes_unseen_tab"]="Biomes Unseen"
    wj(MOD/f"assets/{ID}/lang/en_us.json", LANG)
    print(f"blocks={len(BLOCKS)} items={len(ITEMS)} total={len(all_ids)}")

if __name__=="__main__": main()
