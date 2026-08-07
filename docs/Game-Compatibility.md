# Game Compatibility

CBE applications in the local validation corpus were run by the standalone emulator for 120 headless frames. Every screenshot below is the RGB565 framebuffer produced by guest execution. If an application stops, times out, or leaves a single-color framebuffer, the batch does not create a screenshot. A successful startup capture does not guarantee that every screen or gameplay path works correctly.

Tested on 2026-08-07. This document is maintained manually after reviewing the batch results and screenshots.

## Supported Application Profile

The current core recognizes little- and big-endian ARM/Thumb CBE executables designed for a 240×400 display, including variable segment headers and fixed-address manager-directory variants. It implements the firmware subsets needed for memory blocks, native data packages, image and text drawing, screen changes, file queries, timers, and keypad input.

Validated behavior includes executable initialization, startup and narrative screens, resource-image decoding, Chinese text and HUD rendering, keypad input, and continued frame execution. Headless capture preserves a valid guest-rendered framebuffer if a later callback stops.

## Summary

| Capture result | Count |
| --- | ---: |
| ✅ Guest-rendered frame | 75 |
| ❌ No screenshot | 0 |
| **Total** | **75** |

## Application List

| # | Application | Screenshot | Capture result |
| ---: | --- | --- | --- |
| 1 | 暴打小猪 | <img src="images/暴打小猪.png" width="120"> | ✅ Rendered, 2648 colors |
| 2 | 暴力摩托 | <img src="images/暴力摩托.png" width="120"> | ✅ Rendered, 119 colors |
| 3 | 捕鱼猎人 | <img src="images/捕鱼猎人.png" width="120"> | ✅ Rendered, 650 colors |
| 4 | 打地鼠 | <img src="images/打地鼠.png" width="120"> | ✅ Rendered, 744 colors |
| 5 | 打火机 | <img src="images/打火机.png" width="120"> | ✅ Rendered, 181 colors |
| 6 | 大家来数钱 | <img src="images/大家来数钱.png" width="120"> | ✅ Rendered, 6015 colors |
| 7 | 电子邮件 | <img src="images/电子邮件.png" width="120"> | ✅ Rendered, 182 colors |
| 8 | 动感骰子 | <img src="images/动感骰子.png" width="120"> | ✅ Rendered, 270 colors |
| 9 | 恶魔城 | <img src="images/恶魔城.png" width="120"> | ✅ Rendered, 11 colors |
| 10 | 恶魔城登录版 | <img src="images/恶魔城登录版.png" width="120"> | ✅ Rendered, 6 colors |
| 11 | 法老祖玛2 | <img src="images/法老祖玛2.png" width="120"> | ✅ Rendered, 62 colors |
| 12 | 愤怒的小鸟 | <img src="images/愤怒的小鸟.png" width="120"> | ✅ Rendered, 241 colors |
| 13 | 疯狂捕鸟 | <img src="images/疯狂捕鸟.png" width="120"> | ✅ Rendered, 199 colors |
| 14 | 疯狂斗地主 | <img src="images/疯狂斗地主.png" width="120"> | ✅ Rendered, 157 colors |
| 15 | 疯狂企鹅大冒险 | <img src="images/疯狂企鹅大冒险.png" width="120"> | ✅ Rendered, 2 colors |
| 16 | 割绳子 | <img src="images/割绳子.png" width="120"> | ✅ Rendered, 784 colors |
| 17 | 割绳子冬季版 | <img src="images/割绳子冬季版.png" width="120"> | ✅ Rendered, 518 colors |
| 18 | 孤岛 | <img src="images/孤岛.png" width="120"> | ✅ Rendered, 508 colors |
| 19 | 鬼吹灯 | <img src="images/鬼吹灯.png" width="120"> | ✅ Rendered, 347 colors |
| 20 | 果蔬连连看 | <img src="images/果蔬连连看.png" width="120"> | ✅ Rendered, 1402 colors |
| 21 | 皇牌空战 | <img src="images/皇牌空战.png" width="120"> | ✅ Rendered, 266 colors |
| 22 | 火辣美女视频 | <img src="images/火辣美女视频.png" width="120"> | ✅ Rendered, 445 colors |
| 23 | 机场指挥部 | <img src="images/机场指挥部.png" width="120"> | ✅ Rendered, 482 colors |
| 24 | 激情砖块 | <img src="images/激情砖块.png" width="120"> | ✅ Rendered, 5701 colors |
| 25 | 极品飞车2012 | <img src="images/极品飞车2012.png" width="120"> | ✅ Rendered, 255 colors |
| 26 | 江湖OL | <img src="images/江湖OL.png" width="120"> | ✅ Rendered, 6 colors |
| 27 | 江湖Online | <img src="images/江湖Online.png" width="120"> | ✅ Rendered, 6 colors |
| 28 | 僵尸先生 | <img src="images/僵尸先生.png" width="120"> | ✅ Rendered, 4 colors |
| 29 | 开心大富翁 | <img src="images/开心大富翁.png" width="120"> | ✅ Rendered, 255 colors |
| 30 | 雷电 | <img src="images/雷电.png" width="120"> | ✅ Rendered, 503 colors |
| 31 | 雷霆战机 | <img src="images/雷霆战机.png" width="120"> | ✅ Rendered, 9 colors |
| 32 | 马戏团 | <img src="images/马戏团.png" width="120"> | ✅ Rendered, 417 colors |
| 33 | 猫和老鼠 | <img src="images/猫和老鼠.png" width="120"> | ✅ Rendered, 150 colors |
| 34 | 美女桌球 | <img src="images/美女桌球.png" width="120"> | ✅ Rendered, 331 colors |
| 35 | 魔鬼理发师 | <img src="images/魔鬼理发师.png" width="120"> | ✅ Rendered, 76 colors |
| 36 | 魔兽塔防 | <img src="images/魔兽塔防.png" width="120"> | ✅ Rendered, 131 colors |
| 37 | 魔塔 | <img src="images/魔塔.png" width="120"> | ✅ Rendered, 145 colors |
| 38 | 牧场物语 | <img src="images/牧场物语.png" width="120"> | ✅ Rendered, 439 colors |
| 39 | 碰嘭球 | <img src="images/碰嘭球.png" width="120"> | ✅ Rendered, 5855 colors |
| 40 | 枪之荣誉 | <img src="images/枪之荣誉.png" width="120"> | ✅ Rendered, 406 colors |
| 41 | 热辣美图 | <img src="images/热辣美图.png" width="120"> | ✅ Rendered, 161 colors |
| 42 | 忍者跳跃 | <img src="images/忍者跳跃.png" width="120"> | ✅ Rendered, 422 colors |
| 43 | 三国群殴传 | <img src="images/三国群殴传.png" width="120"> | ✅ Rendered, 9692 colors |
| 44 | 时间同步 | <img src="images/时间同步.png" width="120"> | ✅ Rendered, 3 colors |
| 45 | 士兵突袭 | <img src="images/士兵突袭.png" width="120"> | ✅ Rendered, 257 colors |
| 46 | 世纪佳缘 | <img src="images/世纪佳缘.png" width="120"> | ✅ Rendered, 435 colors |
| 47 | 水果达人 | <img src="images/水果达人.png" width="120"> | ✅ Rendered, 216 colors |
| 48 | 天气精灵 | <img src="images/天气精灵.png" width="120"> | ✅ Rendered, 20 colors |
| 49 | 涂鸦跳跃 | <img src="images/涂鸦跳跃.png" width="120"> | ✅ Rendered, 187 colors |
| 50 | 歪歪猫发条城历险记V100 | <img src="images/歪歪猫发条城历险记V100.png" width="120"> | ✅ Rendered, 2 colors |
| 51 | 万年历 | <img src="images/万年历.png" width="120"> | ✅ Rendered, 138 colors |
| 52 | 武林外传(新品) | <img src="images/武林外传(新品).png" width="120"> | ✅ Rendered, 2 colors |
| 53 | 武林外传V10 | <img src="images/武林外传V10.png" width="120"> | ✅ Rendered, 3 colors |
| 54 | 吸血鬼猎人 | <img src="images/吸血鬼猎人.png" width="120"> | ✅ Rendered, 373 colors |
| 55 | 现代情趣大全 | <img src="images/现代情趣大全.png" width="120"> | ✅ Rendered, 256 colors |
| 56 | 消息盒子 | <img src="images/消息盒子.png" width="120"> | ✅ Rendered, 481 colors |
| 57 | 小酷 | <img src="images/小酷.png" width="120"> | ✅ Rendered, 241 colors |
| 58 | 小鸟愤怒冬季版 | <img src="images/小鸟愤怒冬季版.png" width="120"> | ✅ Rendered, 256 colors |
| 59 | 笑死人 | <img src="images/笑死人.png" width="120"> | ✅ Rendered, 434 colors |
| 60 | 新闻 | <img src="images/新闻.png" width="120"> | ✅ Rendered, 65 colors |
| 61 | 幸运扑克机 | <img src="images/幸运扑克机.png" width="120"> | ✅ Rendered, 664 colors |
| 62 | 性爱宝典 | <img src="images/性爱宝典.png" width="120"> | ✅ Rendered, 6 colors |
| 63 | 性爱高手 | <img src="images/性爱高手.png" width="120"> | ✅ Rendered, 3 colors |
| 64 | 雄霸天下 | <img src="images/雄霸天下.png" width="120"> | ✅ Rendered, 6 colors |
| 65 | 炫酷音乐彩铃 | <img src="images/炫酷音乐彩铃.png" width="120"> | ✅ Rendered, 444 colors |
| 66 | 血剑Online | <img src="images/血剑Online.png" width="120"> | ✅ Rendered, 6 colors |
| 67 | 移淘网 | <img src="images/移淘网.png" width="120"> | ✅ Rendered, 360 colors |
| 68 | 英汉词典 | <img src="images/英汉词典.png" width="120"> | ✅ Rendered, 336 colors |
| 69 | 在线书城 | <img src="images/在线书城.png" width="120"> | ✅ Rendered, 156 colors |
| 70 | 在线音乐 | <img src="images/在线音乐.png" width="120"> | ✅ Rendered, 51 colors |
| 71 | 战争机器 | <img src="images/战争机器.png" width="120"> | ✅ Rendered, 480 colors |
| 72 | 众神之战 | <img src="images/众神之战.png" width="120"> | ✅ Rendered, 3 colors |
| 73 | 钻石迷情3 | <img src="images/钻石迷情3.png" width="120"> | ✅ Rendered, 951 colors |
| 74 | AppStore | <img src="images/AppStore.png" width="120"> | ✅ Rendered, 173 colors |
| 75 | Google地图 | <img src="images/Google地图.png" width="120"> | ✅ Rendered, 22 colors |

## Known Limitations

- Audio and MIDI playback are not implemented.
- Save states and persistent storage are not implemented.
- Libretro exports are still a scaffold and are not a usable frontend.
- The full 75-application validation corpus produces usable guest-rendered startup frames.
- Fixed-address big-endian lifecycle and less frequently used firmware services remain partial.
- Some successful captures contain only an early loading screen, dialog, or minimal startup UI.
- SCE/MAP/XSE resource parsers are inspection helpers; native executables run through the CPU core and service bridge.
- Compatibility with other resolutions and engine revisions is not guaranteed.

## Reporting a Compatibility Issue

Include the application resolution, the last visible screen, the input that triggers the problem, and the error text. When possible, reproduce it with `cbe_boot` and a short sequence of `--key-event FRAME:KEY` options. Do not attach copyrighted game packages to public issue reports.

The `cbe_boot` tool runs the same machine core without opening a window. A key event uses `FRAME:PHONE_KEY` syntax.

```bash
cargo run --release -p nicaiemu-tools --bin cbe_boot -- \
  path/to/game.CBE --frames 120 --key-event 1:14 --screenshot frame.png
```

Set `CBE_TRACE=all` to trace every bridged service, or provide comma-separated service filters such as `CBE_TRACE=4:24,6:3`. Tracing is disabled by default.
