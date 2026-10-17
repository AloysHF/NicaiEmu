# Game Compatibility

## Current Status

The local validation corpus contains 245 distinct CBE files in three groups: A (123), B
(74), and C (48). Group identifiers distinguish archive variants, including
applications with the same name. Game files are supplied separately and are not
included in this repository. Table filenames use group-relative paths such as
`A/`, `B/`, and `C/`. These collection groups do not identify handset models.
Byte-identical archives are retained once,
with group priority B, then C, then A; differently encoded variants remain separate.

The tables below record the current core's 200-frame idle execution and captured
RGB565 frame. Each callback permits 100,000,000 instructions; each file has a
90-second wall-clock limit. An execution result is not a gameplay rating.
`Ready` means the machine remains active; `Halted` means guest execution stopped.
The current CoolbarEmu captures contain 243 `Ready` and two `Halted` results,
with no execution failures or timeouts across the 245 files.
A captured image may be a loading screen or prompt. Blank frames and guest
assertion halts do not establish usable startup. Interaction is unverified unless
explicitly listed in the Coverage / limitation column. Bookstore B retains its verified offline failure prompt and deliberate timed exit;
online reading remains unverified. Zombie Master V101 reaches its landscape
title menu. Music (recommended) V101 stops after offline continuation; usable
startup is unverified. SMS Recharge V101 reaches its numeric account/password
form. Six online-game V101 variants reach login screens. Authentication, online
rooms, actual recharges and multiplayer games remain unavailable.

## Summary

| Group | Files | Execution completed | Execution failed | Timed out |
| --- | ---: | ---: | ---: | ---: |
| A | 123 | 123 | 0 | 0 |
| B | 74 | 74 | 0 | 0 |
| C | 48 | 48 | 0 | 0 |

Verified interaction includes Wulin's menus and battle actions, one Crazy
Landlord round, War Chess's menu/help/purchase panels, Westward Journey's title
menu, the QCIF Ebook bookshelf, and Love Assistant V10 list menus and article
selection/back navigation, plus IM login rendering and field-focus navigation.
Fish Classic title/menu and help navigation are also checked.
Zombie Master (recommended variant) title/menu, touch selection, help text and
return navigation are checked with its supplied companion resources.
1943 V101 reaches aircraft/equipment selection and battle through local point-play
billing simulation. War Chess V101 displays the same local receipt; its battle
path remains unverified. Flying Eagle V101 still reports payment failure.
In 1943, acknowledge the receipt with the right soft key (`E`), then use the
left soft key (`Q`) to open equipment and confirm the sortie.
Servers are treated as unavailable. Continuation timers used after offline channel
or HTTP POST errors advance without retry waits to avoid prolonged startup update
prompts. Normal game timers and GET error handling retain their pace.
Variant-specific coverage is listed below.
Fifteen embedded-program variants restore normal title/menu rendering and basic
selection through 2,000-frame input checks. Active but blank/loading variants
remain incomplete, as specified in their individual rows.
Complete playthroughs and all audio paths remain unverified; billing and network
callbacks are local offline simulations.

## Application List

Screenshots use separate A/B/C image directories to avoid filename collisions.
The Display column records the active output dimensions after orientation;
startup clips take precedence over inferred clears. Unidentified modes retain
the 240×400 fallback. Correct dimensions alone do not establish correct rendering.
“Required” denotes an established network dependency; “Not assessed” means the
dependency has not been verified for that archive variant. Offline callbacks do
not provide the original server's content or a real carrier payment service.

### Group A

| # | Application / file | Display | Screenshot | Network | Execution | Coverage / limitation |
| ---: | --- | --- | --- | --- | --- | --- |
| 1 | `A/1943命运之决战V101.CBE` | 240×320 | <img src="images/A/1943命运之决战V101.png" width="120" alt="1943命运之决战V101 idle frame"> | Local billing simulation | Ready | Local point-play receipt, aircraft/equipment selection, battle start, directional movement, firing and retry flow checked; sequential review replayed battle/movement/fire at 205 frames and retry at 560 frames without faults; full campaign and audio unverified |
| 2 | `A/三国大富翁(新品).CBE` | 240×320 | <img src="images/A/三国大富翁(新品).png" width="120" alt="三国大富翁(新品) idle frame"> | Not assessed | Ready | Title/menu, help text, scrolling and back navigation checked for 2,000 frames; complete gameplay unverified |
| 3 | `A/三国大富翁V10.CBE` | 240×320 | <img src="images/A/三国大富翁V10.png" width="120" alt="三国大富翁V10 idle frame"> | Not assessed | Ready | Image palette colors checked; interaction/gameplay unverified |
| 4 | `A/三国志(新品)V10.CBE` | 240×320 | <img src="images/A/三国志(新品)V10.png" width="120" alt="三国志(新品)V10 idle frame"> | Not assessed | Ready | Menu, introduction and purchase-failure recovery checked; paid battle unverified |
| 5 | `A/三国志@火.CBE` | 240×320 | <img src="images/A/三国志@火.png" width="120" alt="三国志@火 idle frame"> | Not assessed | Ready | Loading message renders; basic startup incomplete |
| 6 | `A/三国志V101.CBE` | 240×320 | <img src="images/A/三国志V101.png" width="120" alt="三国志V101 idle frame"> | Local receipt; battle unverified | Ready | Title/menu, help and game introduction checked; local point-play receipt renders; battle unverified |
| 7 | `A/俄罗斯方块(免费).CBE` | 240×320 | <img src="images/A/俄罗斯方块(免费).png" width="120" alt="俄罗斯方块(免费) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 8 | `A/傲气雄鹰V101.CBE` | 240×320 | <img src="images/A/傲气雄鹰V101.png" width="120" alt="傲气雄鹰V101 idle frame"> | Offline menu; paid play unavailable | Ready | Offline title/menu, help, fee prompt, payment failure and return navigation checked; paid gameplay remains blocked by offline billing |
| 9 | `A/僵尸先生(强推).CBE` | 320×240 | <img src="images/A/僵尸先生(强推).png" width="120" alt="僵尸先生(强推) idle frame"> | No known requirement | Ready | Title/menu, touch selection, help text and return verified for 2,000 frames with supplied companion resources; CBE-only installation and complete gameplay unverified |
| 10 | `A/僵尸先生(新品)V10.CBE` | 320×240 | <img src="images/A/僵尸先生(新品)V10.png" width="120" alt="僵尸先生(新品)V10 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 11 | `A/僵尸先生V101.CBE` | 320×240 | <img src="images/A/僵尸先生V101.png" width="120" alt="僵尸先生V101 idle frame"> | Offline menu; online features unavailable | Ready | Landscape title/menu restored with supplied companion resources; offline network-error panel checked; complete gameplay unverified |
| 12 | `A/动感保龄球.CBE` | 240×320 | <img src="images/A/动感保龄球.png" width="120" alt="动感保龄球 idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 13 | `A/卧底风云(精品).CBE` | 240×320 | <img src="images/A/卧底风云(精品).png" width="120" alt="卧底风云(精品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 14 | `A/卧底风云V10.CBE` | 240×320 | <img src="images/A/卧底风云V10.png" width="120" alt="卧底风云V10 idle frame"> | Not assessed | Ready | Cover/purchase-failure screen checked; paid gameplay unverified |
| 15 | `A/卧底风云V101.CBE` | 240×320 | <img src="images/A/卧底风云V101.png" width="120" alt="卧底风云V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 16 | `A/原罪天使V101.CBE` | 240×320 | <img src="images/A/原罪天使V101.png" width="120" alt="原罪天使V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 17 | `A/双截龙(新品).CBE` | 240×320 | <img src="images/A/双截龙(新品).png" width="120" alt="双截龙(新品) idle frame"> | Not assessed | Ready | Title restored; starting play still fails during loading |
| 18 | `A/双截龙V10.CBE` | 240×320 | <img src="images/A/双截龙V10.png" width="120" alt="双截龙V10 idle frame"> | Not assessed | Ready | Image palette colors and title/introduction checked; entering gameplay faults in incomplete Scene/Actor interfaces |
| 19 | `A/变形金刚(新品).CBE` | 240×320 | <img src="images/A/变形金刚(新品).png" width="120" alt="变形金刚(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 20 | `A/合金装备(新品).CBE` | 240×320 | <img src="images/A/合金装备(新品).png" width="120" alt="合金装备(新品) idle frame"> | Not assessed | Ready | Title content renders; continuation input and gameplay unverified |
| 21 | `A/合金装备V10.CBE` | 240×320 | <img src="images/A/合金装备V10.png" width="120" alt="合金装备V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 22 | `A/合金装备V101.CBE` | 240×320 | <img src="images/A/合金装备V101.png" width="120" alt="合金装备V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 23 | `A/喜羊羊与灰太狼@新.CBE` | 240×320 | <img src="images/A/喜羊羊与灰太狼@新.png" width="120" alt="喜羊羊与灰太狼@新 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 24 | `A/喜羊羊与灰太狼V10.CBE` | 240×320 | <img src="images/A/喜羊羊与灰太狼V10.png" width="120" alt="喜羊羊与灰太狼V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 25 | `A/噗噜噗噜旋转棒V101.CBE` | 240×320 | <img src="images/A/噗噜噗噜旋转棒V101.png" width="120" alt="噗噜噗噜旋转棒V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 26 | `A/在线书城(新品).CBE` | 240×320 | <img src="images/A/在线书城(新品).png" width="120" alt="在线书城(新品) idle frame"> | Required for online content | Ready | Title, toolbar and directional selection verified for 2,000 frames; online content and request completion unverified |
| 27 | `A/在线书城V10.CBE` | 240×320 | <img src="images/A/在线书城V10.png" width="120" alt="在线书城V10 idle frame"> | Not assessed | Ready | Book background/icons and usage prompt render; font/layout differences remain; interaction unverified |
| 28 | `A/坦克大战(新品).CBE` | 240×320 | <img src="images/A/坦克大战(新品).png" width="120" alt="坦克大战(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 29 | `A/坦克大战V10.CBE` | 240×320 | <img src="images/A/坦克大战V10.png" width="120" alt="坦克大战V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 30 | `A/坦克大战V101.CBE` | 240×320 | <img src="images/A/坦克大战V101.png" width="120" alt="坦克大战V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 31 | `A/士兵突袭(精品).CBE` | 240×320 | <img src="images/A/士兵突袭(精品).png" width="120" alt="士兵突袭(精品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 32 | `A/士兵突袭V10.CBE` | 240×320 | <img src="images/A/士兵突袭V10.png" width="120" alt="士兵突袭V10 idle frame"> | Not assessed | Ready | Cover/introduction/purchase checked; paid gameplay unverified |
| 33 | `A/大唐双龙(新品)V101.CBE` | 240×320 | <img src="images/A/大唐双龙(新品)V101.png" width="120" alt="大唐双龙(新品)V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 34 | `A/大话西游(新品).CBE` | 240×400 | <img src="images/A/大话西游(新品).png" width="120" alt="大话西游(新品) idle frame"> | Not assessed | Ready | Background/menu and input checked; later animation/content incomplete |
| 35 | `A/大话西游V10.CBE` | 240×320 | <img src="images/A/大话西游V10.png" width="120" alt="大话西游V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 36 | `A/太空堡垒(新品).CBE` | 240×320 | <img src="images/A/太空堡垒(新品).png" width="120" alt="太空堡垒(新品) idle frame"> | Not assessed | Ready | Title/menu labels render; interaction and gameplay unverified |
| 37 | `A/太空堡垒V10.CBE` | 240×320 | <img src="images/A/太空堡垒V10.png" width="120" alt="太空堡垒V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 38 | `A/封神演义(新品).CBE` | 240×320 | <img src="images/A/封神演义(新品).png" width="120" alt="封神演义(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 39 | `A/封神演义V10.CBE` | 240×320 | <img src="images/A/封神演义V10.png" width="120" alt="封神演义V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 40 | `A/封神演义V101.CBE` | 240×320 | <img src="images/A/封神演义V101.png" width="120" alt="封神演义V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 41 | `A/小酷V10.CBE` | 240×320 | <img src="images/A/小酷V10.png" width="120" alt="小酷V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 42 | `A/小酷V101.CBE` | 240×320 | <img src="images/A/小酷V101.png" width="120" alt="小酷V101 idle frame"> | Required | Ready | Offline startup advances past update prompt; waiting screen, timeout popup and single dismissal checked; subsequent Back can fault, and message text and usable online content are missing |
| 43 | `A/小酷书城(免费看书)V101.CBE` | 240×320 | <img src="images/A/小酷书城(免费看书)V101.png" width="120" alt="小酷书城(免费看书)V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 44 | `A/小酷农场(火爆上线)V10.CBE` | 240×320 | <img src="images/A/小酷农场(火爆上线)V10.png" width="120" alt="小酷农场(火爆上线)V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 45 | `A/小酷音乐(强推)V101.CBE` | 240×320 | <img src="images/A/小酷音乐(强推)V101.png" width="120" alt="小酷音乐(强推)V101 idle frame"> | Not assessed | Halted | Guest stops after offline continuation; usable startup and online content unverified |
| 46 | `A/开心花园(新品).CBE` | 240×320 | <img src="images/A/开心花园(新品).png" width="120" alt="开心花园(新品) idle frame"> | Not assessed | Ready | Title/menu, help text and basic navigation verified for 2,000 frames; complete gameplay unverified |
| 47 | `A/恋爱助手V10.CBE` | 240×320 | <img src="images/A/恋爱助手V10.png" width="120" alt="恋爱助手V10 idle frame"> | Not assessed | Ready | List menu, submenu, article selection, scrolling and back navigation verified; billing callbacks are simulated offline |
| 48 | `A/恶魔城(新品).CBE` | 240×320 | <img src="images/A/恶魔城(新品).png" width="120" alt="恶魔城(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 49 | `A/恶魔城V10.CBE` | 240×320 | <img src="images/A/恶魔城V10.png" width="120" alt="恶魔城V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 50 | `A/战争机器V101.CBE` | 240×320 | <img src="images/A/战争机器V101.png" width="120" alt="战争机器V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 51 | `A/战火军旗V10.CBE` | 240×320 | <img src="images/A/战火军旗V10.png" width="120" alt="战火军旗V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 52 | `A/战火军棋(新品).CBE` | 240×320 | <img src="images/A/战火军棋(新品).png" width="120" alt="战火军棋(新品) idle frame"> | Not assessed | Ready | Title, menu, help, purchase panel and keypad/touch checked; paid battles unverified |
| 53 | `A/拆弹专家V101.CBE` | 240×320 | <img src="images/A/拆弹专家V101.png" width="120" alt="拆弹专家V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 54 | `A/掌上书城V10.CBE` | 240×320 | <img src="images/A/掌上书城V10.png" width="120" alt="掌上书城V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 55 | `A/掌上地图V10.CBE` | 240×320 | <img src="images/A/掌上地图V10.png" width="120" alt="掌上地图V10 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 56 | `A/星际宝藏(新品).CBE` | 240×320 | <img src="images/A/星际宝藏(新品).png" width="120" alt="星际宝藏(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 57 | `A/星际宝藏V10.CBE` | 240×320 | <img src="images/A/星际宝藏V10.png" width="120" alt="星际宝藏V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 58 | `A/暴力摩托(新品).CBE` | 240×320 | <img src="images/A/暴力摩托(新品).png" width="120" alt="暴力摩托(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 59 | `A/暴力摩托V10.CBE` | 240×320 | <img src="images/A/暴力摩托V10.png" width="120" alt="暴力摩托V10 idle frame"> | Not assessed | Ready | Menu renders at 240x320; gameplay unverified |
| 60 | `A/极品飞车(火爆).CBE` | 240×320 | <img src="images/A/极品飞车(火爆).png" width="120" alt="极品飞车(火爆) idle frame"> | Not assessed | Ready | Startup, menu navigation, help, back and touch input verified; starting a race still hits a guest picture-library assertion |
| 61 | `A/极品飞车V10.CBE` | 240×320 | <img src="images/A/极品飞车V10.png" width="120" alt="极品飞车V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 62 | `A/极限挑战V101.CBE` | 240×320 | <img src="images/A/极限挑战V101.png" width="120" alt="极限挑战V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 63 | `A/毒蛇行动V101.CBE` | 240×320 | <img src="images/A/毒蛇行动V101.png" width="120" alt="毒蛇行动V101 idle frame"> | Offline menu; paid play unverified | Ready | Title/menu, character-selection screen, return navigation and fee prompt checked; paid gameplay remains unverified |
| 64 | `A/江湖Online(大型网游)V10.CBE` | 240×320 | <img src="images/A/江湖Online(大型网游)V10.png" width="120" alt="江湖Online(大型网游)V10 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 65 | `A/法老祖玛(新品).CBE` | 240×320 | <img src="images/A/法老祖玛(新品).png" width="120" alt="法老祖玛(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 66 | `A/法老祖玛V10.CBE` | 240×320 | <img src="images/A/法老祖玛V10.png" width="120" alt="法老祖玛V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 67 | `A/漂亮小护士(新品).CBE` | 240×320 | <img src="images/A/漂亮小护士(新品).png" width="120" alt="漂亮小护士(新品) idle frame"> | Not assessed | Ready | Title/menu restored; selected continuation still fails during loading |
| 68 | `A/漂亮小护士V10.CBE` | 240×320 | <img src="images/A/漂亮小护士V10.png" width="120" alt="漂亮小护士V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 69 | `A/漫画大王(新品).CBE` | 240×320 | <img src="images/A/漫画大王(新品).png" width="120" alt="漫画大王(新品) idle frame"> | Not assessed | Ready | Title text renders; comic content and basic functionality unverified |
| 70 | `A/炸弹人(新品).CBE` | 240×320 | <img src="images/A/炸弹人(新品).png" width="120" alt="炸弹人(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 71 | `A/热辣美图(新品).CBE` | 240×320 | <img src="images/A/热辣美图(新品).png" width="120" alt="热辣美图(新品) idle frame"> | Not assessed | Ready | Title, complete usage prompt and picture-browser navigation verified for 2,000 frames; remote downloads unverified |
| 72 | `A/爱之征途V101.CBE` | 240×320 | <img src="images/A/爱之征途V101.png" width="120" alt="爱之征途V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 73 | `A/王牌伞兵(新品).CBE` | 240×320 | <img src="images/A/王牌伞兵(新品).png" width="120" alt="王牌伞兵(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 74 | `A/王牌伞兵V10.CBE` | 240×320 | <img src="images/A/王牌伞兵V10.png" width="120" alt="王牌伞兵V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 75 | `A/生化危机(新品).CBE` | 320×240 | <img src="images/A/生化危机(新品).png" width="120" alt="生化危机(新品) idle frame"> | Not assessed | Ready | Landscape title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 76 | `A/生化危机V10.CBE` | 320×240 | <img src="images/A/生化危机V10.png" width="120" alt="生化危机V10 idle frame"> | Not assessed | Ready | Landscape idle frame captured; interaction/gameplay unverified |
| 77 | `A/生化危机V101.CBE` | 320×240 | <img src="images/A/生化危机V101.png" width="120" alt="生化危机V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 78 | `A/生肖宝贝(新品).CBE` | 240×320 | <img src="images/A/生肖宝贝(新品).png" width="120" alt="生肖宝贝(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 79 | `A/生肖运程V10.CBE` | 240×320 | <img src="images/A/生肖运程V10.png" width="120" alt="生肖运程V10 idle frame"> | Not assessed | Ready | Image palette colors checked; interaction/gameplay unverified |
| 80 | `A/疯狂斗地主(新品).CBE` | 240×320 | <img src="images/A/疯狂斗地主(新品).png" width="120" alt="疯狂斗地主(新品) idle frame"> | Not assessed | Ready | Menu, help, card actions and one round result checked; all rules/rooms/audio unverified |
| 81 | `A/疯狂斗地主V10.CBE` | 240×320 | <img src="images/A/疯狂斗地主V10.png" width="120" alt="疯狂斗地主V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 82 | `A/移动书城V10.CBE` | 240×320 | <img src="images/A/移动书城V10.png" width="120" alt="移动书城V10 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 83 | `A/穿越火线V101.CBE` | 240×320 | <img src="images/A/穿越火线V101.png" width="120" alt="穿越火线V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 84 | `A/绝密宝藏(新品).CBE` | 240×320 | <img src="images/A/绝密宝藏(新品).png" width="120" alt="绝密宝藏(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 85 | `A/绝秘宝藏V10.CBE` | 240×320 | <img src="images/A/绝秘宝藏V10.png" width="120" alt="绝秘宝藏V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 86 | `A/绝秘宝藏V101.CBE` | 240×320 | <img src="images/A/绝秘宝藏V101.png" width="120" alt="绝秘宝藏V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 87 | `A/美女俄罗斯V10.CBE` | 240×320 | <img src="images/A/美女俄罗斯V10.png" width="120" alt="美女俄罗斯V10 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 88 | `A/美女拖拉机V10.CBE` | 240×320 | <img src="images/A/美女拖拉机V10.png" width="120" alt="美女拖拉机V10 idle frame"> | Not assessed | Ready | Title, menus and scene navigation verified for 2,000 frames; complete gameplay unverified |
| 89 | `A/联网五子棋(新版)V101.CBE` | 240×320 | <img src="images/A/联网五子棋(新版)V101.png" width="120" alt="联网五子棋(新版)V101 idle frame"> | Required | Ready | Login screen and numeric account/password fields checked for 2,000 idle callbacks plus scripted edits; online authentication and play unavailable |
| 90 | `A/联网扎金花(新版)V101.CBE` | 240×320 | <img src="images/A/联网扎金花(新版)V101.png" width="120" alt="联网扎金花(新版)V101 idle frame"> | Required | Ready | Login screen and numeric account/password fields checked for 2,000 idle callbacks plus scripted edits; online authentication and play unavailable |
| 91 | `A/联网拖拉机(新版)V101.CBE` | 240×320 | <img src="images/A/联网拖拉机(新版)V101.png" width="120" alt="联网拖拉机(新版)V101 idle frame"> | Required | Ready | Login screen and numeric account/password fields checked for 2,000 idle callbacks plus scripted edits; online authentication and play unavailable |
| 92 | `A/联网斗地主(新版)V101.CBE` | 240×320 | <img src="images/A/联网斗地主(新版)V101.png" width="120" alt="联网斗地主(新版)V101 idle frame"> | Required | Ready | Login screen and numeric account/password fields checked for 2,000 idle callbacks plus scripted edits; online authentication and play unavailable |
| 93 | `A/联网泡泡龙(新版)V101.CBE` | 240×320 | <img src="images/A/联网泡泡龙(新版)V101.png" width="120" alt="联网泡泡龙(新版)V101 idle frame"> | Required | Ready | Login screen and numeric account/password fields checked for 2,000 idle callbacks plus scripted edits; online authentication and play unavailable |
| 94 | `A/联网麻将(强推)V101.CBE` | 240×320 | <img src="images/A/联网麻将(强推)V101.png" width="120" alt="联网麻将(强推)V101 idle frame"> | Required | Ready | Login screen and numeric account/password fields checked for 2,000 idle callbacks plus scripted edits; online authentication and play unavailable |
| 95 | `A/象棋之唐伯虎点秋香V10.CBE` | 240×320 | <img src="images/A/象棋之唐伯虎点秋香V10.png" width="120" alt="象棋之唐伯虎点秋香V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 96 | `A/贪吃鱼(经典).CBE` | 240×320 | <img src="images/A/贪吃鱼(经典).png" width="120" alt="贪吃鱼(经典) idle frame"> | Not assessed | Ready | Title/menu, help text and basic selection checked; complete gameplay unverified |
| 97 | `A/超级八十分(新品).CBE` | 240×320 | <img src="images/A/超级八十分(新品).png" width="120" alt="超级八十分(新品) idle frame"> | Not assessed | Ready | Title, menus and selection verified for 2,000 frames; full card gameplay unverified |
| 98 | `A/超级泡泡龙(新品).CBE` | 240×320 | <img src="images/A/超级泡泡龙(新品).png" width="120" alt="超级泡泡龙(新品) idle frame"> | Touch / keys | Ready | Title/menu, help/back, level selection and game board verified through 2,000 frames; dismiss the startup offer before navigating. Firing effects, full gameplay and paid progression unverified |
| 99 | `A/超级泡泡龙V10.CBE` | 240×320 | <img src="images/A/超级泡泡龙V10.png" width="120" alt="超级泡泡龙V10 idle frame"> | Touch / keys | Ready | Title/menu, help/back, game board and numeric-5 firing with bubble removal/score increase verified; repeated input through 2,000 frames remains Ready. Full gameplay and paid progression unverified |
| 100 | `A/超级玛丽(新品).CBE` | 240×320 | <img src="images/A/超级玛丽(新品).png" width="120" alt="超级玛丽(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 101 | `A/超级玛丽V10.CBE` | 240×320 | <img src="images/A/超级玛丽V10.png" width="120" alt="超级玛丽V10 idle frame"> | Not assessed | Ready | Title renders; 300-frame confirm replay completes; gameplay completeness unverified |
| 102 | `A/超级麻将(新品).CBE` | 240×320 | <img src="images/A/超级麻将(新品).png" width="120" alt="超级麻将(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 103 | `A/超级麻将V10.CBE` | 240×320 | <img src="images/A/超级麻将V10.png" width="120" alt="超级麻将V10 idle frame"> | Not assessed | Ready | Title/menu and room selection verified for 2,000 frames; full rounds and online rooms unverified |
| 104 | `A/酷宝充值(短信直充)V101.CBE` | 240×320 | <img src="images/A/酷宝充值(短信直充)V101.png" width="120" alt="酷宝充值(短信直充)V101 idle frame"> | Required | Ready | Account/password form, numeric editing and password masking checked; online login and actual recharge unavailable |
| 105 | `A/金刚大战V10.CBE` | 240×320 | <img src="images/A/金刚大战V10.png" width="120" alt="金刚大战V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 106 | `A/金字塔传奇(新品).CBE` | 240×320 | <img src="images/A/金字塔传奇(新品).png" width="120" alt="金字塔传奇(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 107 | `A/金字塔传奇V101.CBE` | 240×320 | <img src="images/A/金字塔传奇V101.png" width="120" alt="金字塔传奇V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 108 | `A/陆小凤传奇V101.CBE` | 240×320 | <img src="images/A/陆小凤传奇V101.png" width="120" alt="陆小凤传奇V101 idle frame"> | Offline opening checked | Ready | Title/menu, selection, opening narrative and skip into the first character dialogue checked for up to 2,000 callbacks; saved-game continuation and full gameplay unverified |
| 109 | `A/雷霆战机(新品).CBE` | 240×320 | <img src="images/A/雷霆战机(新品).png" width="120" alt="雷霆战机(新品) idle frame"> | Not assessed | Ready | Loading prompt/ranking panel renders; normal startup incomplete |
| 110 | `A/雷霆战机V10.CBE` | 240×320 | <img src="images/A/雷霆战机V10.png" width="120" alt="雷霆战机V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 111 | `A/马戏团(新品).CBE` | 240×320 | <img src="images/A/马戏团(新品).png" width="120" alt="马戏团(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 112 | `A/马戏团V10.CBE` | 240×320 | <img src="images/A/马戏团V10.png" width="120" alt="马戏团V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 113 | `A/马戏团V101.CBE` | 240×320 | <img src="images/A/马戏团V101.png" width="120" alt="马戏团V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 114 | `A/魔兽塔防(新品).CBE` | 240×320 | <img src="images/A/魔兽塔防(新品).png" width="120" alt="魔兽塔防(新品) idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 115 | `A/魔兽塔防V10.CBE` | 240×320 | <img src="images/A/魔兽塔防V10.png" width="120" alt="魔兽塔防V10 idle frame"> | Not assessed | Ready | Title/menu, help and return verified for 2,000 frames; entering gameplay still shows a blank map |
| 116 | `A/魔力弹球(新品).CBE` | 240×320 | <img src="images/A/魔力弹球(新品).png" width="120" alt="魔力弹球(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 117 | `A/魔塔(新品).CBE` | 240×320 | <img src="images/A/魔塔(新品).png" width="120" alt="魔塔(新品) idle frame"> | Not assessed | Ready | Start/trial confirmation, first-floor map, stairs, NPC and touch movement; 2,000-frame repeated-tap check; brief black frames during entry remain; later floors/combat unverified |
| 118 | `A/魔塔V10.CBE` | 240×320 | <img src="images/A/魔塔V10.png" width="120" alt="魔塔V10 idle frame"> | Not assessed | Ready | First-floor map, stairs, NPC and touch movement; 2,000-frame repeated-tap check; later floors/combat unverified |
| 119 | `A/魔幻英雄(新品).CBE` | 240×320 | <img src="images/A/魔幻英雄(新品).png" width="120" alt="魔幻英雄(新品) idle frame"> | Not assessed | Ready | Title/menu rendering and basic selection checked for 2,000 frames; complete gameplay unverified |
| 120 | `A/魔幻英雄(新品)V10.CBE` | 240×320 | <img src="images/A/魔幻英雄(新品)V10.png" width="120" alt="魔幻英雄(新品)V10 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 121 | `A/魔幻英雄V101.CBE` | 240×320 | <img src="images/A/魔幻英雄V101.png" width="120" alt="魔幻英雄V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |
| 122 | `A/鹿鼎记(新品推荐)V10.CBE` | 240×320 | <img src="images/A/鹿鼎记(新品推荐)V10.png" width="120" alt="鹿鼎记(新品推荐)V10 idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 123 | `A/龙王战士V101.CBE` | 240×320 | <img src="images/A/龙王战士V101.png" width="120" alt="龙王战士V101 idle frame"> | Not assessed | Ready | Offline continuation frame captured; interaction/gameplay unverified |

### Group B

| # | Application / file | Display | Screenshot | Network | Execution | Coverage / limitation |
| ---: | --- | --- | --- | --- | --- | --- |
| 1 | `B/AppStore.CBE` | 240×400 | <img src="images/B/AppStore.png" width="120" alt="AppStore idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 2 | `B/Google地图.CBE` | 240×400 | <img src="images/B/Google地图.png" width="120" alt="Google地图 idle frame"> | Required | Ready | Packaged background renders; firmware-provided terms/title text unavailable; online operation unverified |
| 3 | `B/万年历.CBE` | 240×400 | <img src="images/B/万年历.png" width="120" alt="万年历 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 4 | `B/三国群殴传.CBE` | 400×240 | <img src="images/B/三国群殴传.png" width="120" alt="三国群殴传 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 5 | `B/世纪佳缘.CBE` | 240×400 | <img src="images/B/世纪佳缘.png" width="120" alt="世纪佳缘 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 6 | `B/众神之战.CBE` | 240×400 | <img src="images/B/众神之战.png" width="120" alt="众神之战 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 7 | `B/僵尸先生.CBE` | 400×240 | <img src="images/B/僵尸先生.png" width="120" alt="僵尸先生 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 8 | `B/割绳子.CBE` | 240×400 | <img src="images/B/割绳子.png" width="120" alt="割绳子 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 9 | `B/割绳子冬季版.CBE` | 240×400 | <img src="images/B/割绳子冬季版.png" width="120" alt="割绳子冬季版 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 10 | `B/动感骰子.CBE` | 240×400 | <img src="images/B/动感骰子.png" width="120" alt="动感骰子 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 11 | `B/吸血鬼猎人.CBE` | 400×240 | <img src="images/B/吸血鬼猎人.png" width="120" alt="吸血鬼猎人 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 12 | `B/在线书城.CBE` | 240×400 | <img src="images/B/在线书城.png" width="120" alt="在线书城 offline error prompt"> | Required | Halted | Offline failure prompt and normal timed exit verified; scrolling/font differences remain, online reading unverified |
| 13 | `B/在线音乐.CBE` | 240×400 | <img src="images/B/在线音乐.png" width="120" alt="在线音乐 idle frame"> | Required | Ready | Search controls, toolbars and add prompt render; prompt scrolls with current font metrics; online operation unverified |
| 14 | `B/士兵突袭.CBE` | 400×240 | <img src="images/B/士兵突袭.png" width="120" alt="士兵突袭 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 15 | `B/大家来数钱.CBE` | 240×400 | <img src="images/B/大家来数钱.png" width="120" alt="大家来数钱 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 16 | `B/天气精灵.CBE` | 240×400 | <img src="images/B/天气精灵.png" width="120" alt="天气精灵 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 17 | `B/孤岛.CBE` | 240×400 | <img src="images/B/孤岛.png" width="120" alt="孤岛 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 18 | `B/小酷.CBE` | 240×400 | <img src="images/B/小酷.png" width="120" alt="小酷 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 19 | `B/小鸟愤怒冬季版.CBE` | 400×240 | <img src="images/B/小鸟愤怒冬季版.png" width="120" alt="小鸟愤怒冬季版 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 20 | `B/幸运扑克机.CBE` | 400×240 | <img src="images/B/幸运扑克机.png" width="120" alt="幸运扑克机 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 21 | `B/开心大富翁.CBE` | 400×240 | <img src="images/B/开心大富翁.png" width="120" alt="开心大富翁 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 22 | `B/忍者跳跃.CBE` | 240×400 | <img src="images/B/忍者跳跃.png" width="120" alt="忍者跳跃 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 23 | `B/性爱宝典.CBE` | 240×400 | <img src="images/B/性爱宝典.png" width="120" alt="性爱宝典 idle frame"> | No known requirement | Ready | Disclaimer title, wrapped body and accept/reject labels render; interaction unverified |
| 24 | `B/性爱高手.CBE` | 240×400 | <img src="images/B/性爱高手.png" width="120" alt="性爱高手 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 25 | `B/恶魔城.CBE` | 240×400 | <img src="images/B/恶魔城.png" width="120" alt="恶魔城 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 26 | `B/恶魔城登录版.CBE` | 240×320 | <img src="images/B/恶魔城登录版.png" width="120" alt="恶魔城登录版 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 27 | `B/愤怒的小鸟.CBE` | 400×240 | <img src="images/B/愤怒的小鸟.png" width="120" alt="愤怒的小鸟 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 28 | `B/战争机器.CBE` | 240×400 | <img src="images/B/战争机器.png" width="120" alt="战争机器 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 29 | `B/打地鼠.CBE` | 240×400 | <img src="images/B/打地鼠.png" width="120" alt="打地鼠 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 30 | `B/打火机.CBE` | 240×400 | <img src="images/B/打火机.png" width="120" alt="打火机 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 31 | `B/捕鱼猎人.CBE` | 400×240 | <img src="images/B/捕鱼猎人.png" width="120" alt="捕鱼猎人 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 32 | `B/新闻.CBE` | 240×400 | <img src="images/B/新闻.png" width="120" alt="新闻 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 33 | `B/时间同步.CBE` | 240×400 | <img src="images/B/时间同步.png" width="120" alt="时间同步 idle frame"> | Required | Ready | Top icon/title missing; prompt renders |
| 34 | `B/暴力摩托.CBE` | 400×240 | <img src="images/B/暴力摩托.png" width="120" alt="暴力摩托 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 35 | `B/暴打小猪.CBE` | 240×400 | <img src="images/B/暴打小猪.png" width="120" alt="暴打小猪 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 36 | `B/机场指挥部.CBE` | 400×240 | <img src="images/B/机场指挥部.png" width="120" alt="机场指挥部 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 37 | `B/极品飞车2012.CBE` | 240×400 | <img src="images/B/极品飞车2012.png" width="120" alt="极品飞车2012 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 38 | `B/果蔬连连看.CBE` | 240×400 | <img src="images/B/果蔬连连看.png" width="120" alt="果蔬连连看 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 39 | `B/枪之荣誉.CBE` | 240×400 | <img src="images/B/枪之荣誉.png" width="120" alt="枪之荣誉 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 40 | `B/武林外传(新品).CBE` | 240×320 | <img src="images/B/武林外传(新品).png" width="120" alt="武林外传(新品) idle frame"> | No known requirement | Ready | Menu, help, characters, map/dialogue and battle input checked; complete playthrough/audio unverified |
| 41 | `B/武林外传V10.CBE` | 240×320 | <img src="images/B/武林外传V10.png" width="120" alt="武林外传V10 idle frame"> | No known requirement | Ready | Image palette colors checked; interaction/gameplay unverified |
| 42 | `B/歪歪猫发条城历险记V100.CBE` | 176×220 | <img src="images/B/歪歪猫发条城历险记V100.png" width="120" alt="歪歪猫发条城历险记V100 idle frame"> | Required | Ready | Title artwork and main menu render; resource decompression completes; gameplay unverified |
| 43 | `B/水果达人.CBE` | 400×240 | <img src="images/B/水果达人.png" width="120" alt="水果达人 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 44 | `B/江湖Online.CBE` | 240×400 | <img src="images/B/江湖Online.png" width="120" alt="江湖Online idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 45 | `B/法老祖玛2.CBE` | 400×240 | <img src="images/B/法老祖玛2.png" width="120" alt="法老祖玛2 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 46 | `B/涂鸦跳跃.CBE` | 240×352 | <img src="images/B/涂鸦跳跃.png" width="120" alt="涂鸦跳跃 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 47 | `B/消息盒子.CBE` | 240×400 | <img src="images/B/消息盒子.png" width="120" alt="消息盒子 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 48 | `B/激情砖块.CBE` | 240×400 | <img src="images/B/激情砖块.png" width="120" alt="激情砖块 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 49 | `B/火辣美女视频.CBE` | 240×400 | <img src="images/B/火辣美女视频.png" width="120" alt="火辣美女视频 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 50 | `B/炫酷音乐彩铃.CBE` | 240×400 | <img src="images/B/炫酷音乐彩铃.png" width="120" alt="炫酷音乐彩铃 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 51 | `B/热辣美图.CBE` | 240×400 | <img src="images/B/热辣美图.png" width="120" alt="热辣美图 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 52 | `B/牧场物语.CBE` | 240×400 | <img src="images/B/牧场物语.png" width="120" alt="牧场物语 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 53 | `B/猫和老鼠.CBE` | 240×400 | <img src="images/B/猫和老鼠.png" width="120" alt="猫和老鼠 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 54 | `B/现代情趣大全.CBE` | 240×400 | <img src="images/B/现代情趣大全.png" width="120" alt="现代情趣大全 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 55 | `B/电子邮件.CBE` | 240×400 | <img src="images/B/电子邮件.png" width="120" alt="电子邮件 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 56 | `B/疯狂企鹅大冒险.CBE` | 400×240 | <img src="images/B/疯狂企鹅大冒险.png" width="120" alt="疯狂企鹅大冒险 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 57 | `B/疯狂捕鸟.CBE` | 400×240 | <img src="images/B/疯狂捕鸟.png" width="120" alt="疯狂捕鸟 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 58 | `B/疯狂斗地主.CBE` | 400×240 | <img src="images/B/疯狂斗地主.png" width="120" alt="疯狂斗地主 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 59 | `B/皇牌空战.CBE` | 240×400 | <img src="images/B/皇牌空战.png" width="120" alt="皇牌空战 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 60 | `B/碰嘭球.CBE` | 240×400 | <img src="images/B/碰嘭球.png" width="120" alt="碰嘭球 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 61 | `B/移淘网.CBE` | 240×400 | <img src="images/B/移淘网.png" width="120" alt="移淘网 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 62 | `B/笑死人.CBE` | 240×400 | <img src="images/B/笑死人.png" width="120" alt="笑死人 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 63 | `B/美女桌球.CBE` | 400×240 | <img src="images/B/美女桌球.png" width="120" alt="美女桌球 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 64 | `B/英汉词典.CBE` | 240×400 | <img src="images/B/英汉词典.png" width="120" alt="英汉词典 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 65 | `B/血剑Online.CBE` | 240×400 | <img src="images/B/血剑Online.png" width="120" alt="血剑Online idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 66 | `B/钻石迷情3.CBE` | 240×400 | <img src="images/B/钻石迷情3.png" width="120" alt="钻石迷情3 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 67 | `B/雄霸天下.CBE` | 240×400 | <img src="images/B/雄霸天下.png" width="120" alt="雄霸天下 idle frame"> | Required | Ready | Idle frame captured; interaction/gameplay unverified |
| 68 | `B/雷电.CBE` | 240×400 | <img src="images/B/雷电.png" width="120" alt="雷电 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 69 | `B/雷霆战机.CBE` | 240×400 | <img src="images/B/雷霆战机.png" width="120" alt="雷霆战机 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 70 | `B/马戏团.CBE` | 240×400 | <img src="images/B/马戏团.png" width="120" alt="马戏团 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 71 | `B/鬼吹灯.CBE` | 240×400 | <img src="images/B/鬼吹灯.png" width="120" alt="鬼吹灯 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 72 | `B/魔兽塔防.CBE` | 240×400 | <img src="images/B/魔兽塔防.png" width="120" alt="魔兽塔防 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 73 | `B/魔塔.CBE` | 240×400 | <img src="images/B/魔塔.png" width="120" alt="魔塔 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |
| 74 | `B/魔鬼理发师.CBE` | 240×400 | <img src="images/B/魔鬼理发师.png" width="120" alt="魔鬼理发师 idle frame"> | No known requirement | Ready | Idle frame captured; interaction/gameplay unverified |

### Group C

| # | Application / file | Display | Screenshot | Network | Execution | Coverage / limitation |
| ---: | --- | --- | --- | --- | --- | --- |
| 1 | `C/Almanac.CBE` | 176×220 | <img src="images/C/Almanac.png" width="120" alt="Almanac idle frame"> | Not assessed | Ready | Offline error panel renders; interaction and online content unverified |
| 2 | `C/Apotheosize.CBE` | 176×220 | <img src="images/C/Apotheosize.png" width="120" alt="Apotheosize idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 3 | `C/BombMan.CBE` | 176×220 | <img src="images/C/BombMan.png" width="120" alt="BombMan idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 4 | `C/Bowling.CBE` | 176×220 | <img src="images/C/Bowling.png" width="120" alt="Bowling idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 5 | `C/brick.CBE` | 176×220 | <img src="images/C/brick.png" width="120" alt="brick idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 6 | `C/Car.CBE` | 176×220 | <img src="images/C/Car.png" width="120" alt="Car idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 7 | `C/Cartoon.CBE` | 176×220 | <img src="images/C/Cartoon.png" width="120" alt="Cartoon idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 8 | `C/DaimonCastle.CBE` | 176×220 | <img src="images/C/DaimonCastle.png" width="120" alt="DaimonCastle idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 9 | `C/Deluxe.CBE` | 176×220 | <img src="images/C/Deluxe.png" width="120" alt="Deluxe idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 10 | `C/dragon.CBE` | 176×220 | <img src="images/C/dragon.png" width="120" alt="dragon idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 11 | `C/Ebook.CBE` | 176×220 | <img src="images/C/Ebook.png" width="120" alt="Ebook idle frame"> | Not assessed | Ready | Bookshelf/directional selection checked; native 176x220 viewport missing; remote books unavailable |
| 12 | `C/Eden.CBE` | 176×220 | <img src="images/C/Eden.png" width="120" alt="Eden idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 13 | `C/Email.CBE` | 176×220 | <img src="images/C/Email.png" width="120" alt="Email idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 14 | `C/Fighter.CBE` | 176×220 | <img src="images/C/Fighter.png" width="120" alt="Fighter idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 15 | `C/Garden.CBE` | 176×220 | <img src="images/C/Garden.png" width="120" alt="Garden idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 16 | `C/GoldMan.CBE` | 176×220 | <img src="images/C/GoldMan.png" width="120" alt="GoldMan idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 17 | `C/happySheep.CBE` | 176×220 | <img src="images/C/happySheep.png" width="120" alt="happySheep idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 18 | `C/HellRaider.CBE` | 176×220 | <img src="images/C/HellRaider.png" width="120" alt="HellRaider idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 19 | `C/HitLand.CBE` | 176×220 | <img src="images/C/HitLand.png" width="120" alt="HitLand idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 20 | `C/Im.CBE` | 176×220 | <img src="images/C/Im.png" width="120" alt="Im idle frame"> | Required for online login | Ready | Login screen and field-focus navigation checked for 2,000 frames; online authentication unavailable |
| 21 | `C/kriegspiel.CBE` | 176×220 | <img src="images/C/kriegspiel.png" width="120" alt="kriegspiel idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 22 | `C/MaJiang.CBE` | 176×220 | <img src="images/C/MaJiang.png" width="120" alt="MaJiang idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 23 | `C/Map.CBE` | 176×220 | <img src="images/C/Map.png" width="120" alt="Map idle frame"> | Not assessed | Ready | Map grid, center marker and bottom menu render; online map data unverified |
| 24 | `C/mario.CBE` | 176×220 | <img src="images/C/mario.png" width="120" alt="mario idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 25 | `C/MetalSlug.CBE` | 176×220 | <img src="images/C/MetalSlug.png" width="120" alt="MetalSlug idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 26 | `C/MoTa.CBE` | 176×220 | <img src="images/C/MoTa.png" width="120" alt="MoTa idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 27 | `C/MyGirl.CBE` | 176×220 | <img src="images/C/MyGirl.png" width="120" alt="MyGirl idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 28 | `C/netgobang.CBE` | 176×220 | <img src="images/C/netgobang.png" width="120" alt="netgobang idle frame"> | Not assessed | Ready | Native 176×220 LCD; margins resolved; font images/UI content remain incorrect; gameplay unverified |
| 29 | `C/netGoldFlower.CBE` | 176×220 | <img src="images/C/netGoldFlower.png" width="120" alt="netGoldFlower idle frame"> | Not assessed | Ready | Native 176×220 LCD; margins resolved; font images/UI content remain incorrect; gameplay unverified |
| 30 | `C/Picview.CBE` | 176×220 | <img src="images/C/Picview.png" width="120" alt="Picview idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 31 | `C/Pocker.CBE` | 176×220 | <img src="images/C/Pocker.png" width="120" alt="Pocker idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 32 | `C/Precious.CBE` | 176×220 | <img src="images/C/Precious.png" width="120" alt="Precious idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 33 | `C/Rich.CBE` | 176×220 | <img src="images/C/Rich.png" width="120" alt="Rich idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 34 | `C/Ringside.CBE` | 176×220 | <img src="images/C/Ringside.png" width="120" alt="Ringside idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 35 | `C/RussBlock.CBE` | 176×220 | <img src="images/C/RussBlock.png" width="120" alt="RussBlock idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 36 | `C/Sanbing.CBE` | 176×220 | <img src="images/C/Sanbing.png" width="120" alt="Sanbing idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 37 | `C/Star.CBE` | 176×220 | <img src="images/C/Star.png" width="120" alt="Star idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 38 | `C/Stock.CBE` | 176×220 | <img src="images/C/Stock.png" width="120" alt="Stock idle frame"> | Not assessed | Ready | Native 176×220 LCD; packed coordinates corrected; menu no longer clipped; gameplay unverified |
| 39 | `C/SuperAirship.CBE` | 176×220 | <img src="images/C/SuperAirship.png" width="120" alt="SuperAirship idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 40 | `C/Sxbaby.CBE` | 176×220 | <img src="images/C/Sxbaby.png" width="120" alt="Sxbaby idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 41 | `C/TankWar.CBE` | 176×220 | <img src="images/C/TankWar.png" width="120" alt="TankWar idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 42 | `C/TD.CBE` | 176×220 | <img src="images/C/TD.png" width="120" alt="TD idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 43 | `C/TF.CBE` | 176×220 | <img src="images/C/TF.png" width="120" alt="TF idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 44 | `C/Train.CBE` | 176×220 | <img src="images/C/Train.png" width="120" alt="Train idle frame"> | Not assessed | Ready | Native 176×220 LCD; packed coordinates corrected; menu no longer clipped; gameplay unverified |
| 45 | `C/UniversalWar.CBE` | 176×220 | <img src="images/C/UniversalWar.png" width="120" alt="UniversalWar idle frame"> | Not assessed | Ready | Native 176×220 LCD; startup margins resolved; gameplay unverified |
| 46 | `C/Weather.CBE` | 176×220 | <img src="images/C/Weather.png" width="120" alt="Weather idle frame"> | Not assessed | Ready | Idle frame captured; interaction/gameplay unverified |
| 47 | `C/Zombie.CBE` | 220×176 | <img src="images/C/Zombie.png" width="120" alt="Zombie idle frame"> | Not assessed | Ready | Automatic landscape 220×176; LCD margins resolved; gameplay unverified |
| 48 | `C/ZuMa.CBE` | 176×220 | <img src="images/C/ZuMa.png" width="120" alt="ZuMa idle frame"> | Not assessed | Ready | Native 176×220 LCD; packed coordinates corrected; menu no longer clipped; gameplay unverified |

## Known Limitations

- Shared startup templates still reach guest assertions and halt with black frames.
- Native Scene/Actor, Listener/Talker and control interfaces are incomplete;
  a successful idle run does not certify paths entered by subsequent input.
- Network services use offline responses. Remote content, original online sessions
  and carrier billing are unavailable. Guest file writes are not persisted to host
  storage; host sidecar files are loaded read-only.
- Westward Journey's later animation/content and the specific rendering gaps
  listed in the tables remain incomplete. QCIF LCD sizes are resolved, but font
  images/UI content in netgobang and netGoldFlower remain incorrect.
- Complete playthroughs, every character/stage, all audio paths and all resolutions
  have not been validated. File-based MP3 control is incomplete.
- Save states restore guest memory and execution, but host-side virtual files,
  pending network/billing callbacks and the random generator state are not
  serialized. Resuming across these operations is not fully supported.

## Reporting a Compatibility Issue

Include the group, filename, resolution, last visible screen, triggering input
and error text. Reproduce with `cbe_boot` and scripted phone keys where possible.
Do not attach copyrighted game packages to public reports.

```bash
cargo run --release -p coolbaremu-tools --bin cbe_boot -- \
  path/to/game.CBE --frames 200 --key-event 1:14 --screenshot frame.png
```

Use `CBE_TRACE=all` or selected services such as `CBE_TRACE=4:24,6:3` for diagnostics.
