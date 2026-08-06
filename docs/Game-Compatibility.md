# Game Compatibility

CBE applications in the local validation corpus were tested with the headless emulator for 120 frames. A pass means the process completed and produced a frame containing more than one color; it does not guarantee that every screen or gameplay path works correctly.

Tested on 2026-08-06. The compatibility table is maintained manually after reviewing batch screenshots and execution results.

## Supported Application Profile

The current core targets little-endian ARM/Thumb CBE executables designed for a 240×400 display. It supports applications that use the implemented native service subset for packaged images, maps, actors, text, screen changes, and keypad input.

Validated end-to-end behavior includes executable initialization, title and narrative screens, main-screen transitions, compressed actor and image loading, Chinese text and HUD rendering, keypad input, and continued frame execution.

## Summary

| Status | Count |
| --- | ---: |
| ✅ Pass (rendered frame) | 3 |
| ⚠️ Warn (blank or unverified frame) | 0 |
| ❌ Fail (execution error) | 72 |
| **Total** | **75** |

## Application List

| # | Application | File | Screenshot | Status | Result |
| ---: | --- | --- | --- | --- | --- |
| 1 | 暴打小猪 | `暴打小猪.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1E7D4 |
| 2 | 暴力摩托 | `暴力摩托.CBE` | — | ❌ Fail | instruction fetch from unmapped address 0x00000000 |
| 3 | 捕鱼猎人 | `捕鱼猎人.CBE` | — | ❌ Fail | missing CBE segment separator at 0x246E0 |
| 4 | 打地鼠 | `打地鼠.CBE` | — | ❌ Fail | missing CBE segment separator at 0x16E68 |
| 5 | 打火机 | `打火机.CBE` | — | ❌ Fail | missing CBE segment separator at 0x11A04 |
| 6 | 大家来数钱 | `大家来数钱.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1E514 |
| 7 | 电子邮件 | `电子邮件.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1D764 |
| 8 | 动感骰子 | `动感骰子.CBE` | — | ❌ Fail | instruction fetch from unmapped address 0x00000000 |
| 9 | 恶魔城 | `恶魔城.CBE` | — | ❌ Fail | missing CBE segment separator at 0x21FC4 |
| 10 | 恶魔城登录版 | `恶魔城登录版.CBE` | — | ❌ Fail | CBE application has no active screen |
| 11 | 法老祖玛2 | `法老祖玛2.CBE` | — | ❌ Fail | missing CBE segment separator at 0x29928 |
| 12 | 愤怒的小鸟 | `愤怒的小鸟.CBE` | — | ❌ Fail | missing CBE segment separator at 0x21110 |
| 13 | 疯狂捕鸟  | `疯狂捕鸟 .CBE` | — | ❌ Fail | missing CBE segment separator at 0x2299C |
| 14 | 疯狂斗地主 | `疯狂斗地主.CBE` | — | ❌ Fail | missing CBE segment separator at 0x23E68 |
| 15 | 疯狂企鹅大冒险 | `疯狂企鹅大冒险.CBE` | — | ❌ Fail | missing CBE segment separator at 0x176D0 |
| 16 | 割绳子 | `割绳子.CBE` | — | ❌ Fail | missing CBE segment separator at 0x24F68 |
| 17 | 割绳子冬季版 | `割绳子冬季版.CBE` | — | ❌ Fail | missing CBE segment separator at 0x24EB0 |
| 18 | 孤岛 | `孤岛.CBE` | — | ❌ Fail | missing CBE segment separator at 0x2162C |
| 19 | 鬼吹灯 | `鬼吹灯.CBE` | — | ❌ Fail | missing CBE segment separator at 0x26210 |
| 20 | 果蔬连连看  | `果蔬连连看 .CBE` | — | ❌ Fail | CBE code checksum mismatch |
| 21 | 皇牌空战 | `皇牌空战.CBE` | — | ❌ Fail | missing CBE segment separator at 0x25428 |
| 22 | 火辣美女视频 | `火辣美女视频.CBE` | — | ❌ Fail | missing CBE segment separator at 0x7804 |
| 23 | 机场指挥部 | `机场指挥部.CBE` | — | ❌ Fail | missing CBE segment separator at 0x22418 |
| 24 | 激情砖块 | `激情砖块.CBE` | — | ❌ Fail | missing CBE segment separator at 0x3747C |
| 25 | 极品飞车2012 | `极品飞车2012.CBE` | — | ❌ Fail | missing CBE segment separator at 0x28040 |
| 26 | 江湖OL | `江湖OL.cbe` | — | ❌ Fail | missing CBE segment separator at 0x50C50 |
| 27 | 江湖Online | `江湖Online.CBE` | — | ❌ Fail | missing CBE segment separator at 0x50C50 |
| 28 | 僵尸先生 | `僵尸先生.CBE` | — | ❌ Fail | CBE application has no active screen |
| 29 | 开心大富翁 | `开心大富翁.CBE` | — | ❌ Fail | CBE code checksum mismatch |
| 30 | 雷电 | `雷电.CBE` | — | ❌ Fail | missing CBE segment separator at 0x2BCD0 |
| 31 | 雷霆战机 | `雷霆战机.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1B1B4 |
| 32 | 马戏团 | `马戏团.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1E4EC |
| 33 | 猫和老鼠 | `猫和老鼠.CBE` | — | ❌ Fail | missing CBE segment separator at 0x19558 |
| 34 | 美女桌球 | `美女桌球.CBE` | — | ❌ Fail | missing CBE segment separator at 0x25D1C |
| 35 | 魔鬼理发师 | `魔鬼理发师.CBE` | — | ❌ Fail | unsupported ARM instruction at 0x0101FBA8 |
| 36 | 魔兽塔防 | `魔兽塔防.CBE` | — | ❌ Fail | CBE code checksum mismatch |
| 37 | 魔塔 | `魔塔.CBE` | ![魔塔](images/%E9%AD%94%E5%A1%94.png) | ✅ Pass | Rendered 145 colors |
| 38 | 牧场物语 | `牧场物语.CBE` | — | ❌ Fail | missing CBE segment separator at 0x2F4B4 |
| 39 | 碰嘭球 | `碰嘭球.CBE` | — | ❌ Fail | missing CBE segment separator at 0x21934 |
| 40 | 枪之荣誉 | `枪之荣誉.CBE` | ![枪之荣誉](images/%E6%9E%AA%E4%B9%8B%E8%8D%A3%E8%AA%89.png) | ✅ Pass | Rendered 407 colors |
| 41 | 热辣美图 | `热辣美图.CBE` | — | ❌ Fail | missing CBE segment separator at 0xC898 |
| 42 | 忍者跳跃 | `忍者跳跃.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1EF88 |
| 43 | 三国群殴传 | `三国群殴传.CBE` | — | ❌ Fail | CBE code checksum mismatch |
| 44 | 时间同步 | `时间同步.CBE` | — | ❌ Fail | missing CBE segment separator at 0xAABC |
| 45 | 士兵突袭 | `士兵突袭.CBE` | ![士兵突袭](images/%E5%A3%AB%E5%85%B5%E7%AA%81%E8%A2%AD.png) | ✅ Pass | Rendered 26 colors |
| 46 | 世纪佳缘 | `世纪佳缘.CBE` | — | ❌ Fail | missing CBE segment separator at 0x7290 |
| 47 | 水果达人 | `水果达人.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1D938 |
| 48 | 天气精灵 | `天气精灵.CBE` | — | ❌ Fail | missing CBE segment separator at 0x2AFB0 |
| 49 | 涂鸦跳跃 | `涂鸦跳跃.CBE` | — | ❌ Fail | missing CBE segment separator at 0x17988 |
| 50 | 歪歪猫发条城历险记V100 | `歪歪猫发条城历险记V100.CBE` | — | ❌ Fail | big-endian CBE executables are not supported by the ARM core |
| 51 | 万年历 | `万年历.CBE` | — | ❌ Fail | missing CBE segment separator at 0x276BC |
| 52 | 武林外传(新品) | `武林外传(新品).CBE` | — | ❌ Fail | missing CBE segment separator at 0x15594 |
| 53 | 武林外传V10 | `武林外传V10.CBE` | — | ❌ Fail | missing CBE segment separator at 0x12760 |
| 54 | 吸血鬼猎人 | `吸血鬼猎人.CBE` | — | ❌ Fail | missing CBE segment separator at 0x2684C |
| 55 | 现代情趣大全 | `现代情趣大全.CBE` | — | ❌ Fail | missing CBE segment separator at 0xD0E4 |
| 56 | 消息盒子 | `消息盒子.CBE` | — | ❌ Fail | missing CBE segment separator at 0xDDB4 |
| 57 | 小酷 | `小酷.CBE` | — | ❌ Fail | CBE code checksum mismatch |
| 58 | 小鸟愤怒冬季版 | `小鸟愤怒冬季版.CBE` | — | ❌ Fail | missing CBE segment separator at 0x24C80 |
| 59 | 笑死人 | `笑死人.CBE` | — | ❌ Fail | CBE application has no active screen |
| 60 | 新闻 | `新闻.CBE` | — | ❌ Fail | missing CBE segment separator at 0x126D0 |
| 61 | 幸运扑克机 | `幸运扑克机.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1F218 |
| 62 | 性爱宝典 | `性爱宝典.CBE` | — | ❌ Fail | missing CBE segment separator at 0x14DE4 |
| 63 | 性爱高手 | `性爱高手.CBE` | — | ❌ Fail | CBE code checksum mismatch |
| 64 | 雄霸天下 | `雄霸天下.CBE` | — | ❌ Fail | instruction fetch from unmapped address 0x00000000 |
| 65 | 炫酷音乐彩铃 | `炫酷音乐彩铃.CBE` | — | ❌ Fail | missing CBE segment separator at 0x7804 |
| 66 | 血剑Online | `血剑Online.CBE` | — | ❌ Fail | missing CBE segment separator at 0x513AC |
| 67 | 移淘网 | `移淘网.CBE` | — | ❌ Fail | missing CBE segment separator at 0x7978 |
| 68 | 英汉词典 | `英汉词典.CBE` | — | ❌ Fail | CBE application has no active screen |
| 69 | 在线书城 | `在线书城.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1EE04 |
| 70 | 在线音乐 | `在线音乐.CBE` | — | ❌ Fail | missing CBE segment separator at 0x20598 |
| 71 | 战争机器 | `战争机器.CBE` | — | ❌ Fail | CBE code checksum mismatch |
| 72 | 众神之战 | `众神之战.CBE` | — | ❌ Fail | missing CBE segment separator at 0x35E68 |
| 73 | 钻石迷情3 | `钻石迷情3.CBE` | — | ❌ Fail | missing CBE segment separator at 0x1B5A4 |
| 74 | AppStore | `AppStore.CBE` | — | ❌ Fail | missing CBE segment separator at 0x14620 |
| 75 | Google地图 | `Google地图.CBE` | — | ❌ Fail | CBE application has no active screen |

## Known Limitations

- Audio and MIDI playback are not implemented.
- Save states and persistent storage are not implemented.
- Libretro exports are still a scaffold and are not a usable frontend.
- Some firmware service families return neutral fallback values.
- Big-endian executable checksums can be recognized, but big-endian guest execution has not been validated.
- SCE/MAP/XSE resource parsers are inspection helpers; native executables run through the CPU core and service bridge.
- Compatibility with other resolutions and engine revisions is not guaranteed.

## Reporting a Compatibility Issue

Include the application resolution, the last visible screen, the input that triggers the problem, and the error text. When possible, reproduce it with `cbe_boot` and a short sequence of `--key-event FRAME:KEY` options. Do not attach copyrighted game packages to public issue reports.
