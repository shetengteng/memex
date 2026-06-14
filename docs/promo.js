// Memex Promo · IntersectionObserver 触发 reveal + 同步 progress nav + 录屏字幕.
// 关键: 不操作 layout / paint, 只切 class. GPU 合成保证 0 抖动.

(function () {
  const acts = Array.from(document.querySelectorAll(".act[data-act]"));
  const dots = Array.from(document.querySelectorAll(".promo-dot[data-act]"));
  const caption = document.getElementById("narration-caption");

  // 8 段 narration (跟 /tmp/promo-rec/narration.json 一致).
  // v13: 移除 ChatGPT / Copilot / GPT, 只提 Memex 真实支持的 6 个 adapter.
  const NARRATION = {
    1: "你和 Claude Code 聊了一年。\n那次关键的对话——你还找得到吗？Memex,本地 AI 记忆中枢。",
    2: "Claude Code、Cursor、Codex、OpenCode——\n四个工具同时开着,对话散在四处,反复说着同一句话。",
    3: "上周和 Cursor 拍板的方案,今天找不到。\n你的工作日,正在变成 prompt 的复读机。",
    4: "Memex 把所有 AI 对话写到本地 SQLite——\n不上云、不外传、不被收回。",
    5: "装完就开始记。\n六大主流 AI 编辑器自动接入,事件驱动、2 秒入库。",
    6: "几千条对话躺本地,关键词秒级命中。\n日报、周报自动生成。",
    7: "早八点聊 Claude Code,深夜十一点开 Cursor 复盘——\n记忆永远跟着你走。",
    8: "下载、拖到 Applications、敲两行命令。\n三分钟,装回所有 AI 对话。GitHub:shetengteng/memex",
  };

  // ?caption=1 → 录屏模式: 显示字幕, 隐藏 progress nav / skip-link / scroll-hint
  const showCaptions = new URLSearchParams(location.search).get("caption") === "1";
  if (showCaptions) document.body.classList.add("show-captions");

  // 1) 进入视野时给 .act-inner 加 .is-visible, 触发 .reveal 动画
  const revealObserver = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        const inner = entry.target.querySelector(".reveal");
        if (inner) inner.classList.add("is-visible");
      });
    },
    { threshold: 0.35 }
  );
  acts.forEach((act) => revealObserver.observe(act));

  // 2) 当前 act > 50% 在视口时 高亮对应 dot + 切字幕
  const navObserver = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        const id = entry.target.dataset.act;
        dots.forEach((d) => d.classList.toggle("is-active", d.dataset.act === id));
        if (showCaptions && caption) {
          const text = NARRATION[id];
          if (text) {
            caption.textContent = text;
            caption.classList.add("is-visible");
          } else {
            caption.classList.remove("is-visible");
          }
        }
      });
    },
    { threshold: 0.6 }
  );
  acts.forEach((act) => navObserver.observe(act));

  // 3) 键盘上下方向键 + Page Up/Down 直接跳幕
  let scrollLock = false;
  document.addEventListener("keydown", (e) => {
    if (scrollLock) return;
    const current = dots.findIndex((d) => d.classList.contains("is-active"));
    let target = -1;
    if (e.key === "ArrowDown" || e.key === "PageDown") target = Math.min(current + 1, acts.length - 1);
    if (e.key === "ArrowUp" || e.key === "PageUp")     target = Math.max(current - 1, 0);
    if (target < 0) return;
    e.preventDefault();
    scrollLock = true;
    document.getElementById(`act-${target + 1}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
    setTimeout(() => (scrollLock = false), 700);
  });
})();
