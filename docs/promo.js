// Memex Promo · IntersectionObserver 触发 reveal + 同步 progress nav + 录屏字幕.
// 关键: 不操作 layout / paint, 只切 class. GPU 合成保证 0 抖动.

(function () {
  const acts = Array.from(document.querySelectorAll(".act[data-act]"));
  const dots = Array.from(document.querySelectorAll(".promo-dot[data-act]"));
  const caption = document.getElementById("narration-caption");

  // 8 段 narration (跟 /tmp/promo-rec/narration.json 一致).
  // v21: 全段口语化, 段间加过渡词, 像朋友聊天.
  const NARRATION = {
    1: "这个软件太强了!你是不是经常碰到——\n切一下 AI 编辑器,上次聊到哪儿就全忘了？\nMemex,本地 AI 记忆中枢,就是帮你解决这个的。",
    2: "你想想啊——Claude Code、Cursor、Codex、OpenCode,\n四个工具同时开,对话散在四处,你每天都在重复说同一句话。",
    3: "更崩溃的是——上周在 Cursor 拍板的那套方案,\n今天翻了三个窗口、一小时,还是找不到。你已经第七次跟同一个模型讲同一件事了。",
    4: "所以我们做了 Memex。\n它把你和 AI 的每一句对话,直接写到本地 SQLite——\n不上云、不外传、谁也收不回。",
    5: "怎么用呢？很简单。\n你不用改任何习惯——打开 Cursor、敲下回车,后台 2 秒,这段对话就静悄悄躺进本地数据库。",
    6: "想找以前聊过的东西?\n比如「上次那个 retry 策略到底怎么改的?」敲一行关键词,三个月前的决定,瞬间就回到眼前。",
    7: "最妙的是——早八点在 Claude Code 写新功能,\n深夜十一点切到 Cursor 复盘 bug,两个工具看到的,是同一份你。",
    8: "现在就可以装。\n下载、拖到 Applications、敲两行命令——三分钟,你过去所有的 AI 对话,全部回家。",
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
