// Memex Promo · IntersectionObserver 触发 reveal + 同步 progress nav + 录屏字幕.
// 关键: 不操作 layout / paint, 只切 class. GPU 合成保证 0 抖动.

(function () {
  const acts = Array.from(document.querySelectorAll(".act[data-act]"));
  const dots = Array.from(document.querySelectorAll(".promo-dot[data-act]"));
  const caption = document.getElementById("narration-caption");

  // 8 段 narration (跟 /tmp/promo-rec/narration.json 一致).
  // 第一性原理重写: 50-70 字/段, 每段 ≤2 数字 + 1 个具体场景,
  // 用"开发者 → 开发者"对话感, 而不是带货短视频风.
  // 字幕拆两行以避免单行铺到屏幕外.
  const NARRATION = {
    1: "你和 ChatGPT 聊了一年。一年后想找当初的某次对话——你怎么找？\nMemex,MIT 开源的本地 AI 记忆中枢,让你的 AI 对话变成可搜索的本地资料库。",
    2: "Cursor、Claude Code、ChatGPT、Codex——5 个 AI 工具同时开。\n你的对话散在 5 个孤岛,没有一个工具知道你昨天和另一个聊了什么。",
    3: "上周和 Cursor 拍板的方案,今天找不到。\nGPT 三个月前写的脚本,重写一次。你的工作日,正在变成 prompt 的复读机。",
    4: "Memex 把所有 AI 对话写到本地 SQLite 里。\n不上云、不外传、不被服务商收回——AI 时代第一份属于你自己的记忆库。",
    5: "装完就开始记。Claude Code、Cursor、Codex——\n三个主流 AI 编辑器自动接入,事件驱动、2 秒入库,不改你任何使用习惯。",
    6: "几千条对话躺本地,关键词秒级命中。\nInsights 自动生成日报和周报——AI 替你记,也替你回顾。",
    7: "早八点和 Claude 聊架构,深夜十一点开 GPT 复盘——\n切换工具、切换项目,记忆永远跟着你走。",
    8: "下载、拖进 Applications、敲两行命令。\n三分钟,装回属于你自己的 AI 记忆。Memex,MIT 开源、100% 本地。",
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
