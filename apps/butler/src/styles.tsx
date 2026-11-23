// 管家界面样式注入：幂等挂载 <style id="bt-styles">。CSS 文本在 styles-data.ts。
import { useEffect } from "react";
import { CSS } from "./styles-data";

export { CSS };

export function Styles() {
  useEffect(() => {
    if (document.getElementById("bt-styles")) return;
    const el = document.createElement("style");
    el.id = "bt-styles";
    el.textContent = CSS;
    document.head.appendChild(el);
  }, []);
  return null;
}
