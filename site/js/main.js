// NicaiEmu site interactions: i18n toggle, reveal animations, counters,
// hero screenshot, and the game gallery.

(function () {
  "use strict";

  var REPO = "https://raw.githubusercontent.com/jiangxincode/NicaiEmu/master";

  // All portrait (240x400) screenshots under docs/images/, featured first.
  var PORTRAIT = [
    "魔塔",
    "孤岛",
    "众神之战",
    "鬼吹灯",
    "雷霆战机",
    "暴打小猪",
    "打地鼠",
    "打火机",
    "大家来数钱",
    "电子邮件",
    "动感骰子",
    "恶魔城",
    "恶魔城登录版",
    "割绳子",
    "割绳子冬季版",
    "果蔬连连看",
    "皇牌空战",
    "火辣美女视频",
    "激情砖块",
    "极品飞车2012",
    "江湖Online",
    "雷电",
    "马戏团",
    "猫和老鼠",
    "魔鬼理发师",
    "魔兽塔防",
    "牧场物语",
    "碰嘭球",
    "枪之荣誉",
    "热辣美图",
    "忍者跳跃",
    "时间同步",
    "世纪佳缘",
    "天气精灵",
    "涂鸦跳跃",
    "歪歪猫发条城历险记V100",
    "万年历",
    "武林外传(新品)",
    "武林外传V10",
    "现代情趣大全",
    "消息盒子",
    "小酷",
    "笑死人",
    "新闻",
    "性爱宝典",
    "性爱高手",
    "雄霸天下",
    "炫酷音乐彩铃",
    "血剑Online",
    "移淘网",
    "英汉词典",
    "在线书城",
    "在线音乐",
    "战争机器",
    "钻石迷情3",
    "AppStore",
    "Google地图",
  ];

  // All landscape (400x240) screenshots under docs/images/, featured first.
  var LANDSCAPE = [
    "暴力摩托",
    "捕鱼猎人",
    "愤怒的小鸟",
    "僵尸先生",
    "水果达人",
    "法老祖玛2",
    "疯狂捕鸟",
    "疯狂斗地主",
    "疯狂企鹅大冒险",
    "机场指挥部",
    "开心大富翁",
    "美女桌球",
    "三国群殴传",
    "士兵突袭",
    "吸血鬼猎人",
    "小鸟愤怒冬季版",
    "幸运扑克机",
  ];

  var I18N = {
    "nav.features": { zh: "核心特性", en: "Features" },
    "nav.gallery": { zh: "游戏画廊", en: "Gallery" },
    "nav.arch": { zh: "技术架构", en: "Architecture" },
    "nav.quickstart": { zh: "快速开始", en: "Quick Start" },
    "hero.lead": {
      zh: "NicaiEmu 用 Rust 真实执行 CBE 应用的 ARM/Thumb 代码，并桥接固件服务。同一套核心同时驱动桌面窗口与 RetroArch 前端。",
      en: "NicaiEmu runs the ARM/Thumb code of CBE apps for real and bridges the firmware services. One core drives both the desktop window and the RetroArch frontend.",
    },
    "hero.download": { zh: "下载 Release", en: "Download Release" },
    "hero.source": { zh: "查看源码 ↗", en: "View Source ↗" },
    "hero.note": { zh: "NicaiEmu 实际运行画面", en: "Actual NicaiEmu output" },
    "hero.title": {
      zh: "让尘封的<span class=\"accent\">手机游戏</span><br>在现代设备上重生",
      en: "Bring forgotten <span class=\"accent\">phone games</span><br>back to life on modern devices",
    },
    "about.title": { zh: "什么是 Nicai / MStar 游戏？", en: "What are Nicai / MStar games?" },
    "about.p1": {
      zh: "Nicai（尼采）是 MStar 时代功能手机的游戏平台。游戏以 <strong>.CBE</strong> 容器打包，内含 ARM/Thumb 可执行代码与场景、地图、图片、音频等资源，通过厂商固件服务与硬件交互。这类游戏一度只能在特定手机上运行。",
      en: "Nicai was the game platform on MStar feature phones. Games ship as <strong>.CBE</strong> containers holding ARM/Thumb code plus scene, map, image, and audio resources, talking to the hardware through vendor firmware services. They once ran only on specific handsets.",
    },
    "stats.games": { zh: "语料验证", en: "Games Verified" },
    "stats.gamesSub": { zh: "启动帧 100% 出图", en: "100% boot-frame output" },
    "stats.display": { zh: "原生分辨率", en: "Native Display" },
    "stats.displaySub": { zh: "WQVGA 竖屏", en: "WQVGA portrait" },
    "stats.frontends": { zh: "运行前端", en: "Frontends" },
    "stats.frontendsSub": { zh: "Standalone + RetroArch", en: "Standalone + RetroArch" },
    "stats.audio": { zh: "音频通路", en: "Audio Formats" },
    "stats.audioSub": { zh: "WAV / MP3 / MIDI", en: "WAV / MP3 / MIDI" },
    "features.title": { zh: "核心特性", en: "Core Features" },
    "features.sub": { zh: "真实执行，而非资源预览", en: "Real execution, not a preview" },
    "features.cpu": { zh: "真实 ARM 执行", en: "Real ARM Execution" },
    "features.cpuDesc": {
      zh: "大小端 ARM/Thumb 解释执行，含 interworking 分支与编译器跳转表，运行真实客机代码。",
      en: "Little- and big-endian ARM/Thumb interpretation with interworking branches and compiler jump tables.",
    },
    "features.bridge": { zh: "固件服务桥", en: "Firmware Service Bridge" },
    "features.bridgeDesc": {
      zh: "内存、屏幕、输入、定时器、UCS2 文本、下载/支付等 20+ 服务组，逐调用可追踪。",
      en: "20+ service groups for memory, display, input, timers, UCS2 text, downloads, and payments, traceable per call.",
    },
    "features.audio": { zh: "音频引擎", en: "Audio Engine" },
    "features.audioDesc": {
      zh: "WAV/MP3 解码与 MIDI 合成，44.1 kHz 立体声混音，音量可调并输出确定性诊断。",
      en: "WAV/MP3 decoding and MIDI synthesis, 44.1 kHz stereo mixing, volume control, and deterministic diagnostics.",
    },
    "features.savestate": { zh: "存档与内存", en: "Save States & Memory" },
    "features.savestateDesc": {
      zh: "校验和存档、共享重置，并通过 libretro 向前端暴露客机内存与屏幕缓冲。",
      en: "Checksummed save states, shared reset, and guest memory exposed to frontends through libretro.",
    },
    "features.frontend": { zh: "桌面体验", en: "Desktop Experience" },
    "features.frontendDesc": {
      zh: "4 种缩放滤镜、按键重映射、虚拟手柄 overlay、全屏/音量/headless。",
      en: "Four scaling filters, key remapping, a virtual gamepad overlay, and fullscreen/volume/headless options.",
    },
    "features.libretro": { zh: "RetroArch 核心", en: "RetroArch Core" },
    "features.libretroDesc": {
      zh: "RGB888 输出、RetroPad 映射、指针/触摸输入、音频与存档，即插即用。",
      en: "RGB888 output, RetroPad mapping, pointer/touch input, audio, and save states, ready to use.",
    },
    "gallery.title": { zh: "游戏画廊", en: "Game Gallery" },
    "gallery.sub": { zh: "74 款本地语料 · 每帧都由客机真实渲染", en: "74 local titles · every frame rendered by the guest" },
    "gallery.portrait": { zh: "竖版", en: "Portrait" },
    "gallery.landscape": { zh: "横版", en: "Landscape" },
    "gallery.more": { zh: "完整列表见 Game Compatibility 文档", en: "See the Game Compatibility doc for the full list" },
    "arch.title": { zh: "技术架构", en: "Architecture" },
    "arch.sub": { zh: "平台无关核心，双前端共享同一模拟逻辑", en: "Platform-independent core shared by two frontends" },
    "arch.frontends": { zh: "前端", en: "Frontends" },
    "arch.standalone": { zh: "桌面窗口 · minifb / rodio", en: "Desktop window · minifb / rodio" },
    "arch.libretro": { zh: "cdylib · RetroArch", en: "cdylib · RetroArch" },
    "arch.core": { zh: "核心引擎", en: "Core Engine" },
    "arch.platforms": { zh: "目标平台", en: "Platforms" },
    "qs.title": { zh: "快速开始", en: "Quick Start" },
    "qs.standalone": { zh: "桌面运行", en: "Standalone" },
    "qs.s1": { zh: "从 Releases 下载对应平台二进制", en: "Download the binary for your platform from Releases" },
    "qs.s3": { zh: "方向键移动 · Enter 确认 · Q/E 软键 · R 重置", en: "Arrows move · Enter confirms · Q/E soft keys · R resets" },
    "qs.retro": { zh: "RetroArch", en: "RetroArch" },
    "qs.r1": { zh: "下载 libretro 核心并放入 cores/", en: "Download the libretro core into cores/" },
    "qs.r2": { zh: "放入 nicaiemu_libretro.info 到 info/", en: "Put nicaiemu_libretro.info into info/" },
    "qs.build": { zh: "从源码编译", en: "Build from Source" },
    "footer.desc": { zh: "用 Rust 编写的 Nicai/MStar CBE 游戏模拟器", en: "A Rust emulator for Nicai/MStar CBE games" },
    "footer.project": { zh: "项目", en: "Project" },
    "footer.contrib": { zh: "贡献指南", en: "Contributing" },
    "footer.docs": { zh: "文档", en: "Docs" },
    "footer.standalone": { zh: "独立模拟器", en: "Standalone Emulator" },
    "footer.retro": { zh: "RetroArch 核心", en: "RetroArch Core" },
    "footer.gamelist": { zh: "游戏兼容性", en: "Game Compatibility" },
  };

  var currentLang = localStorage.getItem("nicai-site-lang") || "zh";

  function applyLang(lang) {
    document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
    var toggle = document.getElementById("lang-toggle");
    if (toggle) {
      toggle.textContent = lang === "zh" ? "EN" : "中文";
    }
    document.querySelectorAll("[data-i18n]").forEach(function (node) {
      var key = node.getAttribute("data-i18n");
      var entry = I18N[key];
      if (entry && entry[lang]) {
        node.innerHTML = entry[lang];
      }
    });
  }

  function setupLangToggle() {
    var toggle = document.getElementById("lang-toggle");
    if (!toggle) {
      return;
    }
    toggle.addEventListener("click", function () {
      currentLang = currentLang === "zh" ? "en" : "zh";
      localStorage.setItem("nicai-site-lang", currentLang);
      applyLang(currentLang);
    });
  }

  function setupReveal() {
    var elements = document.querySelectorAll(".section-title, .section-sub, .feature-card, .qs-card, .arch-box, .stat");
    elements.forEach(function (element) {
      element.classList.add("reveal");
    });
    var observer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (entry.isIntersecting) {
            entry.target.classList.add("visible");
            observer.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.12 }
    );
    elements.forEach(function (element) {
      observer.observe(element);
    });
  }

  function setupCounters() {
    var observer = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (entry) {
          if (!entry.isIntersecting) {
            return;
          }
          var node = entry.target;
          observer.unobserve(node);
          var target = parseInt(node.getAttribute("data-count"), 10);
          var suffix = node.getAttribute("data-suffix") || "";
          var start = performance.now();
          var duration = 900;
          function tick(now) {
            var progress = Math.min((now - start) / duration, 1);
            var eased = 1 - Math.pow(1 - progress, 3);
            node.textContent = Math.round(target * eased) + suffix;
            if (progress < 1) {
              requestAnimationFrame(tick);
            }
          }
          requestAnimationFrame(tick);
        });
      },
      { threshold: 0.5 }
    );
    document.querySelectorAll(".stat-num").forEach(function (node) {
      observer.observe(node);
    });
  }

  function imageUrl(name) {
    return REPO + "/docs/images/" + encodeURIComponent(name) + ".png";
  }

  function setupHeroShot() {
    var img = document.getElementById("hero-shot");
    if (img) {
      img.src = imageUrl("魔塔");
      img.alt = "NicaiEmu running 魔塔";
    }
  }

  function setupGallery() {
    var grid = document.getElementById("gallery-grid");
    if (!grid) {
      return;
    }
    var groups = [
      { key: "gallery.portrait", cssClass: "", games: PORTRAIT },
      { key: "gallery.landscape", cssClass: "landscape", games: LANDSCAPE },
    ];
    grid.innerHTML = "";
    groups.forEach(function (group) {
      if (group.games.length === 0) {
        return;
      }
      var groupEl = document.createElement("div");
      groupEl.className = group.cssClass
        ? "gallery-group " + group.cssClass
        : "gallery-group";
      var title = document.createElement("h3");
      title.className = "gallery-group-title";
      title.setAttribute("data-i18n", group.key);
      title.textContent = I18N[group.key][currentLang];
      groupEl.appendChild(title);
      groupEl.appendChild(buildCarousel(group));
      grid.appendChild(groupEl);
    });
    initCarousel();
  }

  function buildCarousel(group) {
    var wrapper = document.createElement("div");
    wrapper.className = "carousel-wrapper";

    var prev = document.createElement("button");
    prev.type = "button";
    prev.className = "carousel-btn carousel-prev";
    prev.setAttribute("aria-label", "Previous");
    prev.innerHTML = "&#8249;";

    var viewport = document.createElement("div");
    viewport.className = "carousel-viewport";

    var track = document.createElement("div");
    track.className = "carousel-track";
    group.games.forEach(function (name) {
        var figure = document.createElement("figure");
        figure.className = group.cssClass
          ? "gallery-item " + group.cssClass
          : "gallery-item";
        var img = document.createElement("img");
        img.loading = "lazy";
        img.src = imageUrl(name);
        img.alt = name;
        var caption = document.createElement("figcaption");
        caption.textContent = name;
        figure.appendChild(img);
        figure.appendChild(caption);
        track.appendChild(figure);
    });
    viewport.appendChild(track);

    var next = document.createElement("button");
    next.type = "button";
    next.className = "carousel-btn carousel-next";
    next.setAttribute("aria-label", "Next");
    next.innerHTML = "&#8250;";

    var dots = document.createElement("div");
    dots.className = "carousel-dots";

    wrapper.appendChild(prev);
    wrapper.appendChild(viewport);
    wrapper.appendChild(next);
    wrapper.appendChild(dots);
    return wrapper;
  }

  function initCarousel() {
    document.querySelectorAll(".carousel-wrapper").forEach(function (wrapper) {
      var viewport = wrapper.querySelector(".carousel-viewport");
      var track = wrapper.querySelector(".carousel-track");
      var prevBtn = wrapper.querySelector(".carousel-prev");
      var nextBtn = wrapper.querySelector(".carousel-next");
      var dotsBox = wrapper.querySelector(".carousel-dots");
      if (!viewport || !track || !dotsBox) {
        return;
      }

      var page = 0;
      var resizeTimer = null;

      function getCardsPerView() {
        return window.innerWidth > 768 ? 4 : (window.innerWidth > 480 ? 2 : 1);
      }

      function getTotalPages() {
        var cards = track.querySelectorAll(".gallery-item");
        return Math.max(1, Math.ceil(cards.length / getCardsPerView()));
      }

      function renderDots() {
        dotsBox.innerHTML = "";
        for (var d = 0; d < getTotalPages(); d++) {
          var dot = document.createElement("span");
          dot.className = "carousel-dot";
          dot.setAttribute("data-page", d);
          (function (idx) {
            dot.addEventListener("click", function () {
              goTo(idx);
            });
          })(d);
          dotsBox.appendChild(dot);
        }
      }

      function goTo(p) {
        var total = getTotalPages();
        page = Math.max(0, Math.min(p, total - 1));
        var cpv = getCardsPerView();
        var card = track.querySelector(".gallery-item");
        var gap = parseFloat(window.getComputedStyle(track).columnGap) || 0;
        var pageWidth = card ? cpv * (card.offsetWidth + gap) : viewport.offsetWidth;
        var maxOffset = Math.max(0, track.scrollWidth - viewport.clientWidth);
        var offset = Math.min(page * pageWidth, maxOffset);
        track.style.transform = "translateX(-" + offset + "px)";

        var dots = dotsBox.querySelectorAll(".carousel-dot");
        dots.forEach(function (dot, i) {
          dot.classList.toggle("active", i === page);
        });
      }

      if (prevBtn) {
        prevBtn.addEventListener("click", function () {
          goTo(page - 1);
        });
      }
      if (nextBtn) {
        nextBtn.addEventListener("click", function () {
          goTo(page + 1);
        });
      }

      // Touch / swipe support.
      var startX = 0;
      viewport.addEventListener(
        "touchstart",
        function (e) {
          startX = e.touches[0].clientX;
        },
        { passive: true }
      );
      viewport.addEventListener(
        "touchend",
        function (e) {
          var diff = startX - e.changedTouches[0].clientX;
          if (Math.abs(diff) > 50) {
            goTo(page + (diff > 0 ? 1 : -1));
          }
        },
        { passive: true }
      );

      // Recompute layout when the viewport changes.
      window.addEventListener("resize", function () {
        clearTimeout(resizeTimer);
        resizeTimer = setTimeout(function () {
          renderDots();
          goTo(page);
        }, 150);
      });

      renderDots();
      goTo(0);
    });
  }

  function setupBurger() {
    var burger = document.querySelector(".nav-burger");
    var links = document.querySelector(".nav-links");
    if (!burger || !links) {
      return;
    }
    burger.addEventListener("click", function () {
      links.classList.toggle("open");
    });
    links.querySelectorAll("a").forEach(function (link) {
      link.addEventListener("click", function () {
        links.classList.remove("open");
      });
    });
  }

  applyLang(currentLang);
  setupLangToggle();
  setupReveal();
  setupCounters();
  setupHeroShot();
  setupGallery();
  setupBurger();
})();
