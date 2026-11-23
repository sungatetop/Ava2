// clock App：主页面显示当前日期与时间（每秒刷新）。
// 裸 ESM 形态，react 由宿主 import map 供给（单实例）；颜色 / 字号一律消费壳令牌 var(--ava-*)，
// 亮暗主题由 documentElement[data-theme] 切换后自动继承（令牌在 src/styles/tokens.css 双套定义）。
import * as React from "react";

const { createElement: h, useEffect, useState } = React;

const clockCss = `
.clock-app-root {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 100%;
  padding: var(--ava-space-5, 24px);
  color: var(--ava-label-primary);
  background: var(--ava-bg-base);
  font-family: var(--ava-font-ui);
}
.clock-app-time {
  font-family: var(--ava-font-mono);
  font-size: 72px;
  line-height: 1.1;
  font-weight: 600;
  letter-spacing: 0.02em;
  margin: 0 0 var(--ava-space-3, 10px);
  color: var(--ava-brand);
}
.clock-app-date {
  font-size: 20px;
  line-height: 1.4;
  margin: 0;
  color: var(--ava-label-secondary);
}
.clock-app-weekday {
  font-size: 14px;
  line-height: 1.6;
  margin: var(--ava-space-2, 8px) 0 0;
  color: var(--ava-label-tertiary);
}`;

const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];

function pad2(n) {
  return String(n).padStart(2, "0");
}

function formatNow(d) {
  const date = `${d.getFullYear()} 年 ${pad2(d.getMonth() + 1)} 月 ${pad2(d.getDate())} 日`;
  const time = `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
  const weekday = `星期${WEEKDAYS[d.getDay()]}`;
  return { date, time, weekday };
}

export function setup(ctx) {
  function Main() {
    const [now, setNow] = useState(() => new Date());

    useEffect(() => {
      let alive = true;
      const timer = setInterval(() => {
        if (alive) setNow(new Date());
      }, 1000);
      return () => {
        alive = false;
        clearInterval(timer);
      };
    }, []);

    const { date, time, weekday } = formatNow(now);

    return h(
      "div",
      { className: "clock-app-root" },
      h("style", null, clockCss),
      h("div", { className: "clock-app-time" }, time),
      h("div", { className: "clock-app-date" }, date),
      h("div", { className: "clock-app-weekday" }, weekday),
    );
  }

  const disposers = [
    ctx.slots.inject("nav.sidebar", { order: 50, viewId: "clock", title: "时钟" }),
    ctx.slots.inject("surface.main", {
      viewId: "clock",
      title: "时钟",
      component: async () => ({ default: Main }),
    }),
  ];

  return {
    dispose() {
      disposers.forEach((d) => d.dispose());
    },
  };
}
