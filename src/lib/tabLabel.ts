/**
 * 标签格显示名：把长文件名压到 N 个字符以内（默认 10），保证网格里一格一词能扫读。
 *
 * 为什么不用 AI 生成缩写：
 *  - 标签是高频切换元素，AI 一次往返就是几百毫秒到数秒，首屏还会闪；
 *  - 同一文件每次打开必须给同一个名字，AI 结果不稳定会让人误认；
 *  - 纯规则算法零延迟、零成本、可预测，10 字以内这个硬约束用规则反而更稳。
 *
 * 规则（按优先级）：
 *  1. 短名本身 ≤ limit：原样返回（只裁扩展名的边界情况）
 *  2. 含分隔符（. - _ 空格）：保留每段首字母缩写，常见命名一眼可辨
 *     SjCommonAuth.java → SCA.java / epointbhcs-service-api → esa
 *  3. 驼峰或长连续串：砍掉元音保辅音骨架（IntelliJ 同款思路）
 *     KnowledgeBaseService → KnwsBssSvc
 *  4. 兜底：首尾保头尾，中间省略号
 */

const VOWELS = new Set(["a", "e", "i", "o", "u"]);

/** 取路径末段的文件名（含扩展名） */
export function baseName(path: string): string {
  return path.replace(/\\/g, "/").split("/").pop() || path;
}

/** 拆扩展名：.tar.gz 这类复合扩展名只拆最后一层 */
function splitExt(name: string): { stem: string; ext: string } {
  const i = name.lastIndexOf(".");
  // 点在开头（.gitignore）或不在前面（无扩展名）时视为无扩展名
  if (i <= 0) return { stem: name, ext: "" };
  return { stem: name.slice(0, i), ext: name.slice(i) };
}

/** 分隔符切段：. - _ 空格、数字边界、以及驼峰边界（KnowledgeBase → Knowledge | Base） */
function segments(stem: string): string[] {
  return stem
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .split(/[.\-_\s]+|(?<=[a-zA-Z])(?=\d)|(?<=\d)(?=[a-zA-Z])/)
    .filter(Boolean);
}

/** 段内缩写成首字母（大写保原样，小写首字母转大写） */
function initialOf(seg: string): string {
  const c = seg[0];
  return c ? c.toUpperCase() : "";
}

/** 驼峰/长串：去元音留辅音骨架；全元音词（api→api）退回原样 */
function skeleton(word: string): string {
  const chars = word.split("");
  const kept = chars.filter((c, i) => {
    const lower = c.toLowerCase();
    if (!VOWELS.has(lower)) return true;
    // 首尾元音保留，避免 "Sj" → "Sj" 被削成 "S"
    if (i === 0 || i === chars.length - 1) return true;
    // 连续元音只留第一个：Ea → Ea
    return i === 0 || !VOWELS.has(chars[i - 1].toLowerCase());
  });
  return kept.join("");
}

/**
 * 生成标签格显示名。
 * @param path 工作区相对路径
 * @param limit 字符上限（含扩展名），默认 10
 */
export function tabLabel(path: string, limit = 10): string {
  const name = baseName(path);
  if (name.length <= limit) return name;

  const { stem, ext } = splitExt(name);

  // 规则 2：多段命名 → 首字母缩写
  const segs = segments(stem);
  if (segs.length >= 2) {
    let abbr = segs.map(initialOf).join("");
    // 缩写后仍超限，就从各段再借一个字符补足可读性
    if (abbr.length + ext.length > limit) {
      let grown = abbr;
      for (const seg of segs) {
        if (grown.length + ext.length > limit) break;
        grown += seg[1] ?? "";
      }
      abbr = grown.slice(0, Math.max(1, limit - ext.length));
    }
    return abbr + ext;
  }

  // 规则 3：驼峰 → 逐词首字母（KnowledgeBaseService → KBS）
  const camel = segments(stem);
  if (camel.length >= 2) {
    let abbr = camel.map(initialOf).join("");
    if (abbr.length + ext.length > limit) {
      abbr = abbr.slice(0, Math.max(1, limit - ext.length));
    }
    return abbr + ext;
  }

  // 规则 4：长连续串 → 去元音保辅音骨架
  const skel = skeleton(stem);
  if (skel.length + ext.length > limit) {
    return stem.slice(0, Math.max(1, limit - ext.length)) + ext;
  }
  return skel + ext;
}