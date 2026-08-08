// NicaiEmu site interactions: i18n toggle, reveal animations, counters,
// hero screenshot, and the game gallery.

(function () {
  "use strict";

  var REPO = "https://raw.githubusercontent.com/jiangxincode/NicaiEmu/master";

  // Selected gallery titles that exist under docs/images/.
  var GALLERY = [
    "魔塔",
    "恶魔城",
    "打地鼠",
    "愤怒的小鸟",
    "割绳子",
    "疯狂斗地主",
    "果蔬连连看",
    "捕鱼猎人",
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
    "about.title": { zh: "什么是 Nicai / MStar 游戏？", en: "What are Nicai / MStar games?" },
    "about.p1": {
      zh: "Nicai（尼采）是 MStar 时代功能手机的游戏平台。游戏以 .CBE 容器打包，内含 ARM/Thumb 可执行代码与场景、地图、图片、音频等资源，通过厂商固件服务与硬件交互。这类游戏一度只能在特定手机上运行。",
      en: "Nicai was the game platform on MStar feature phones. Games ship as .CBE containers holding ARM/Thumb code plus scene, map, image, and audio resources, talking to the hardware through vendor firmware services. They once ran only on specific handsets.",
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
    "gallery.sub": { zh: "75 款本地语料 · 每帧都由客机真实渲染", en: "75 local titles · every frame rendered by the guest" },
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
        node.textContent = entry[lang];
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
    GALLERY.forEach(function (name) {
      var figure = document.createElement("figure");
      figure.className = "gallery-item";
      var img = document.createElement("img");
      img.loading = "lazy";
      img.src = imageUrl(name);
      img.alt = name;
      var caption = document.createElement("figcaption");
      caption.textContent = name;
      figure.appendChild(img);
      figure.appendChild(caption);
      grid.appendChild(figure);
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
