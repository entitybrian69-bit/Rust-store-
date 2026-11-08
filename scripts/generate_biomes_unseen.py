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

# Wood shape variants (templates are vanilla blocks; behaviour/states copy the template)
VAR_TEMPLATES = {
 "stairs":"minecraft:oak_stairs","slab":"minecraft:oak_slab","fence":"minecraft:oak_fence",
 "fence_gate":"minecraft:oak_fence_gate","door":"minecraft:oak_door","trapdoor":"minecraft:oak_trapdoor",
 "button":"minecraft:oak_button","pressure_plate":"minecraft:oak_pressure_plate",
 "sign":"minecraft:oak_sign","wall_sign":"minecraft:oak_wall_sign"}
for w,(base,border,leaf) in WOODS.items():
    disp=w.replace("_"," ").title()
    for suf,tpl in VAR_TEMPLATES.items():
        extra={"item":False} if suf=="wall_sign" else {}
        d={"template":tpl,"properties":{"hardness":2.0,"resistance":3.0,"sounds":"minecraft:wood"}}
        if suf=="sign": d.update({"wood_type":f"{ID}:{w}","wall_block":f"{ID}:{w}_wall_sign"})
        if suf=="wall_sign": d.update({"wood_type":f"{ID}:{w}","item":False})
        BLOCKS.append((f"{w}_{suf}", f"{disp} {suf.replace('_',' ').title()}", d, f"variant:{suf}", (base,border,leaf)))
        LANG[f"block.{ID}.{w}_{suf}"]=f"{disp} {suf.replace('_',' ').title()}"
    item(f"{w}_boat", f"{disp} Boat", "minecraft:oak_boat", {"minecraft:max_stack_size":1}, (base,border), "orb")
    # sign editor backgrounds (24x26 / 16x16 are gui textures; block texture reused as source art)
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


def gen_variant(name, suf, pal):
    """Write vanilla-parent models + full state-variant blockstate for a wood shape."""
    _sufs=["pressure_plate","fence_gate","wall_sign","stairs","slab","fence","door","trapdoor","button","sign"]
    _wood=next((name[:-len(x)-1] for x in _sufs if name.endswith("_"+x)), name)
    P=f"{ID}:block/{_wood}_planks"
    texmap={"bottom":P,"top":P,"side":P,"texture":P,"particle":P,"all":P}
    def model(mname, parent, textures): wj(MOD/f"assets/{ID}/models/block/{mname}.json", {"parent":parent,"textures":textures})
    def bs(variants=None, multipart=None):
        d={}
        if variants is not None: d["variants"]=variants
        if multipart is not None: d["multipart"]=multipart
        wj(MOD/f"assets/{ID}/blockstates/{name}.json", d)
    Y={"east":0,"south":90,"west":180,"north":270}
    if suf=="stairs":
        for m,par in [("","stairs"),("inner","stairs_inner"),("outer","stairs_outer")]: model(f"{name}{('_'+m) if m else ''}", f"minecraft:block/{par}", texmap)
        v={}
        for f,y in Y.items():
            for half in ("bottom","top"):
                for shape in ("straight","inner_left","inner_right","outer_left","outer_right"):
                    mm = name if shape=="straight" else f"{name}_{'inner' if shape.startswith('inner') else 'outer'}"
                    e={"model":f"{ID}:block/{mm}"}
                    yy=y + (90 if shape.endswith("right") else 270 if shape.endswith("left") else 0)
                    if half=="top": e["x"]=180; yy+=90
                    if yy%360: e["y"]=yy%360
                    if e.get("x") or e.get("y"): e["uvlock"]=True
                    v[f"facing={f},half={half},shape={shape}"]=e
        bs(v)
    elif suf=="slab":
        model(name,"minecraft:block/slab",texmap); model(f"{name}_top","minecraft:block/slab_top",texmap); model(f"{name}_double","minecraft:block/cube_all",{"all":P})
        bs({"type=bottom":{"model":f"{ID}:block/{name}"},"type=top":{"model":f"{ID}:block/{name}_top"},"type=double":{"model":f"{ID}:block/{name}_double"}})
    elif suf=="fence":
        model(f"{name}_post","minecraft:block/fence_post",{"texture":P}); model(f"{name}_side","minecraft:block/fence_side",{"texture":P}); model(f"{name}_inventory","minecraft:block/fence_inventory",{"texture":P})
        mp=[{"apply":{"model":f"{ID}:block/{name}_post"}}]
        for side,y in Y.items(): mp.append({"when":{side:"true"},"apply":{"model":f"{ID}:block/{name}_side",**({"y":y} if y else {}),"uvlock":True}})
        bs(multipart=mp)
    elif suf=="fence_gate":
        for m,par in [("","template_fence_gate"),("open","template_fence_gate_open"),("wall","template_fence_gate_wall"),("wall_open","template_fence_gate_wall_open")]: model(f"{name}_{m}" if m else name, f"minecraft:block/{par}", {"texture":P})
        v={}
        for f,y in Y.items():
            for iw in ("false","true"):
                for op in ("false","true"):
                    mm=f"{name}{'_wall' if iw=='true' else ''}{'_open' if op=='true' else ''}"
                    v[f"facing={f},in_wall={iw},open={op},powered=false"]={"model":f"{ID}:block/{mm}",**({"y":y} if y else {}),"uvlock":True}
                    v[f"facing={f},in_wall={iw},open={op},powered=true"]=v[f"facing={f},in_wall={iw},open={op},powered=false"]
        bs(v)
    elif suf=="door":
        for part in ("bottom_left","bottom_right","top_left","top_right"):
            kind="door_bottom" if part.startswith("bottom") else "door_top"
            model(f"{name}_{part}", f"minecraft:block/{kind}", {"bottom":P,"top":P})
        v={}
        for f,y in Y.items():
            for half in ("lower","upper"):
                for hinge in ("left","right"):
                    for op in ("false","true"):
                        mm=f"{name}_{'bottom' if half=='lower' else 'top'}_{hinge}"
                        yy=(y + (90 if (op=='true') != (hinge=='right') else 0))%360
                        e={"model":f"{ID}:block/{mm}"}
                        if yy: e["y"]=yy
                        v[f"facing={f},half={half},hinge={hinge},open={op},powered=false"]=e
                        v[f"facing={f},half={half},hinge={hinge},open={op},powered=true"]=e
        bs(v)
    elif suf=="trapdoor":
        for m in ("bottom","top","open"): model(f"{name}_{m}", f"minecraft:block/trapdoor_{m}", {"texture":P})
        v={}
        for f,y in Y.items():
            for half in ("bottom","top"):
                for op in ("false","true"):
                    mm=f"{name}_open" if op=='true' else f"{name}_{half}"
                    e={"model":f"{ID}:block/{mm}"}
                    yy=(y+180)%360 if op=='true' else y
                    if yy: e["y"]=yy
                    v[f"facing={f},half={half},open={op},powered=false"]=e
                    v[f"facing={f},half={half},open={op},powered=true"]=e
        bs(v)
    elif suf=="button":
        model(name,"minecraft:block/button",{"texture":P}); model(f"{name}_pressed","minecraft:block/button_pressed",{"texture":P}); model(f"{name}_inventory","minecraft:block/button_inventory",{"texture":P})
        v={}
        for face in ("floor","wall","ceiling"):
            for f,y in Y.items():
                for pw in ("false","true"):
                    e={"model":f"{ID}:block/{name}{'_pressed' if pw=='true' else ''}"}
                    if face=="ceiling": e["x"]=180
                    if face!="floor" and y: e["y"]=y
                    if face=="floor" and y: e["y"]=y
                    v[f"face={face},facing={f},powered={pw}"]=e
        bs(v)
    elif suf=="pressure_plate":
        model(name,"minecraft:block/pressure_plate_up",{"up":P,"side":P}); model(f"{name}_down","minecraft:block/pressure_plate_down",{"down":P,"side":P})
        bs({"powered=false":{"model":f"{ID}:block/{name}"},"powered=true":{"model":f"{ID}:block/{name}_down"}})
    elif suf=="sign":
        model(name,"minecraft:block/sign",{"particle":P})
        bs({f"rotation={r}":{"model":f"{ID}:block/{name}",**({"y":int(r*22.5)} if r else {})} for r in range(16)})
    elif suf=="wall_sign":
        model(name,"minecraft:block/wall_sign",{"particle":P})
        bs({f"facing={f}":{"model":f"{ID}:block/{name}",**({"y":y} if y else {})} for f,y in Y.items()})

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
        if kind.startswith("variant:"):
            gen_variant(name, kind.split(":",1)[1], pal)
        elif kind=="log":
            wb(MOD/f"assets/{ID}/textures/block/{name}_top.png", tex(border,base,accent,"cross"))
            model={"parent":"minecraft:block/cube_column","textures":{"end":f"{ID}:block/{name}_top","side":f"{ID}:block/{name}"}}
        elif kind=="cross":
            model={"parent":"minecraft:block/cross","textures":{"cross":f"{ID}:block/{name}"}}
        else:
            model={"parent":"minecraft:block/cube_all","textures":{"all":f"{ID}:block/{name}"}}
        wj(MOD/f"assets/{ID}/models/block/{name}.json", model)
        if not kind.startswith("variant:"):
            wj(MOD/f"assets/{ID}/blockstates/{name}.json", {"variants":{"":{"model":f"{ID}:block/{name}"}}})
        if d.get("item") is not False:
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


    # ---- Worldgen: biomes + configured/placed features (Volt worldgen-api) ----
    import importlib.util as _ilu
    _spec=_ilu.spec_from_file_location("bhn", Path(__file__).with_name("build_hnmod.py")); _bhn=_ilu.module_from_spec(_spec); _spec.loader.exec_module(_bhn)
    def placed(fname, feature, placement): wj(MOD/f"data/{ID}/worldgen/placed_feature/{fname}.json", {"feature":f"{ID}:{feature}","placement":placement})
    def configured(fname, cfg): wj(MOD/f"data/{ID}/worldgen/feature/{fname}.json", cfg)
    COUNT=lambda n:[{"type":"minecraft:count","count":n},{"type":"minecraft:in_square"},{"type":"minecraft:surface_heightmap","heightmap":"OCEAN_FLOOR"},{"type":"minecraft:biome"}]
    # trees per wood
    for w in WOODS:
        configured(f"tree_{w}", {"type":"minecraft:tree","config":{
          "below_trunk_provider":{"type":"minecraft:simple_state_provider","state":{"Name":"minecraft:dirt"}},
          "decorators":[],"ignore_vines":True,
          "foliage_placer":{"type":"minecraft:blob_foliage_placer","radius":2,"offset":0,"height":3},
          "foliage_provider":{"type":"minecraft:simple_state_provider","state":{"Name":f"{ID}:{w}_leaves","Properties":{"persistent":"false","distance":"7","waterlogged":"false"}}},
          "minimum_size":{"type":"minecraft:two_layers_feature_size","limit":1,"lower_size":0,"upper_size":1},
          "trunk_placer":{"type":"minecraft:straight_trunk_placer","base_height":4,"height_rand_a":2,"height_rand_b":0},
          "trunk_provider":{"type":"minecraft:simple_state_provider","state":{"Name":f"{ID}:{w}_log","Properties":{"axis":"y"}}}}})
        placed(f"tree_{w}", f"tree_{w}", COUNT(6))
    for pl in PLANTS:
        configured(f"flora_{pl}", {"type":"minecraft:simple_block","config":{"to_place":{"type":"minecraft:simple_state_provider","state":{"Name":f"{ID}:{pl}"}}}})
        placed(f"flora_{pl}", f"flora_{pl}", COUNT(2))
    configured("ore_tanzanite", {"type":"minecraft:ore","config":{"size":6,"discard_chance_on_air_exposure":0.0,"targets":[
      {"target":{"predicate_type":"minecraft:tag_match","tag":"minecraft:stone_ore_replaceables"},"state":{"Name":f"{ID}:tanzanite_ore"}},
      {"target":{"predicate_type":"minecraft:tag_match","tag":"minecraft:deepslate_ore_replaceables"},"state":{"Name":f"{ID}:tanzanite_ore"}}]}})
    placed("ore_tanzanite","ore_tanzanite",[{"type":"minecraft:count","count":4},{"type":"minecraft:in_square"},{"type":"minecraft:height_range","height":{"type":"minecraft:trapezoid","max_inclusive":{"above_bottom":80},"min_inclusive":{"above_bottom":-20},"plateau":10}},{"type":"minecraft:biome"}])
    # biome -> (tree wood, flora) by category/name heuristics
    WOOD_FOR={"redwood":"redwood","maple":"maple","cherry":"cherry","fir":"fir","dead":"dead","hellbark":"hellbark","umbran":"umbran","magic":"magic","jacaranda":"jacaranda","palm":"palm","willow":"willow","mahogany":"mahogany"}
    for (bn,temp,down,sky,fog,grass,water,weight,cat) in _bhn.BIOMES:
        wood=next((v for k,v in WOOD_FOR.items() if k in bn), "fir" if cat in ("forest","snowy") else "palm" if cat in ("jungle","desert","savanna") else "dead" if cat=="nether" else "willow" if cat=="swamp" else "maple")
        flora = "lavender" if "lavender" in bn else "burning_blossom" if cat=="nether" else "glowshroom" if bn in ("mystic_grove","ominous_woods") else "wildflower"
        steps=[[] for _ in range(11)]
        if cat!="nether": steps[6]=[f"{ID}:ore_tanzanite"]
        steps[9]=[f"{ID}:tree_{wood}", f"{ID}:flora_{flora}"]
        rng=lambda c,sp:(round(max(-2.0,c-sp),3), round(min(2.0,c+sp),3))
        climate={"temperature":list(rng(temp*0.6-0.2,0.35)),"humidity":list(rng(down*1.2-0.6,0.4)),
                 "continentalness":[-0.2,1.0] if cat not in ("beach",) else [-1.2,-0.2],
                 "erosion":list(rng(0.0,0.6)),"depth":[0.0,0.0],"weirdness":list(rng(0.0,0.5)),"offset":0.0}
        wj(MOD/f"data/{ID}/worldgen/biome/{bn}.json", {
          "temperature":temp,"downfall":down,"has_precipitation":down>0.05,
          "effects":{"sky_color":sky,"fog_color":fog,"water_color":water,"water_fog_color":water,"grass_color":grass,"foliage_color":grass},
          "spawners":{},"spawn_costs":{},"carvers":[],"features":steps,
          "horizon:climate":climate, **({"horizon:dimension":"minecraft:the_nether"} if cat=="nether" else {})})
    # biome tags: overworld/nether membership for vanilla systems (locate, etc.)
    wj(MOD/f"data/{ID}/tags/worldgen/biome/is_biomes_unseen.json", {"replace":False,"values":[f"{ID}:{b[0]}" for b in _bhn.BIOMES]})

    wj(MOD/"data/minecraft/tags/block/mineable/axe.json", {"replace":False,"values":sorted(set(axe))})
    wj(MOD/"data/minecraft/tags/block/mineable/pickaxe.json", {"replace":False,"values":sorted(set(pick))})
    all_ids=[f"{ID}:{n}" for n, *_ in BLOCKS]+[f"{ID}:{n}" for n, *_ in ITEMS]
    wj(MOD/f"data/{ID}/horizon/creative_tab/biomes_unseen_tab.json", {"title":"Biomes Unseen","icon":f"{ID}:fir_log","items":all_ids})
    LANG[f"itemGroup.{ID}.biomes_unseen_tab"]="Biomes Unseen"
    wj(MOD/f"assets/{ID}/lang/en_us.json", LANG)
    print(f"blocks={len(BLOCKS)} items={len(ITEMS)} total={len(all_ids)}")

if __name__=="__main__": main()

# ---- appended: variant + worldgen generation (executed from main via hook) ----
