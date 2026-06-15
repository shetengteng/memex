// Memex Promo · IntersectionObserver 触发 reveal + 同步 progress nav + 录屏字幕.
// 关键: 不操作 layout / paint, 只切 class. GPU 合成保证 0 抖动.

(function () {
  const acts = Array.from(document.querySelectorAll(".act[data-act]"));
  const dots = Array.from(document.querySelectorAll(".promo-dot[data-act]"));
  const caption = document.getElementById("narration-caption");

  // 8 段 narration (跟 /tmp/promo-rec/narration.json 一致).
  // v13: 移除 ChatGPT / Copilot / GPT, 只提 Memex 真实支持的 6 个 adapter.
  const NARRATION = {
    1: "这个软件太强了!你是不是经常碰到——\n切一下 AI 编辑器,上次聊到哪儿就全忘了？Memex,本地 AI 记忆中枢。",
    2: "Claude Code、Cursor、Codex、OpenCode——\n四个工具同时开着,对话散在四处,反复说着同一句话。",
    3: "上周在 Cursor 拍板的方案,今天打开三个窗口翻了一小时——还是找不到。\n你正在第七次,跟同一个模型讲同一件事。",
    4: "Memex 把所有 AI 对话写到本地 SQLite——\n不上云、不外传、不被收回。",
    5: "你不用改任何习惯。\n打开 Cursor、敲下回车——后台 2 秒,这段对话就躺进本地数据库。",
    6: "「上次那个 retry 策略,到底怎么改的?」\n敲一行关键词——三个月前的决定,瞬间回到眼前。",
    7: "早八点在 Claude Code 写新功能,深夜十一点切到 Cursor 复盘 bug——\n两个工具看到的,是同一份你。",
    8: "下载、拖到 Applications、敲两行命令。\n三分钟,装回所有 AI 对话。",
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
