// 快速加标签：写回 frontmatter（与属性面板同一真相源：md 文本）
// 性能：YAML 走动态 import —— 这是低频操作（用户点「加标签」），
// 不该让首屏主包为它硬扛 YAML 全量。调用方需 await 本函数。
import { stripFrontmatter } from "./markdown/renderer";

export async function addTagToContent(content: string, tag: string): Promise<string> {
  tag = tag.trim().replace(/^#/, "");
  if (!tag) return content;
  const { fm, body } = stripFrontmatter(content);
  let obj: Record<string, unknown> = {};
  // YAML 为低频依赖，动态载入，避免进首屏主包
  let YAML: typeof import("yaml");
  try {
    YAML = await import("yaml");
  } catch {
    return content;
  }
  if (fm !== null) {
    try {
      obj = (YAML.parse(fm) as Record<string, unknown>) || {};
    } catch {
      return content; // frontmatter 非法时不動
    }
  }
  const v = obj["tags"] ?? obj["tag"];
  let arr: string[] = [];
  if (typeof v === "string") arr = v.split(",").map((s) => s.trim()).filter(Boolean);
  else if (Array.isArray(v)) arr = v.map(String);
  if (!arr.includes(tag)) arr.push(tag);
  obj["tags"] = arr;
  delete obj["tag"];
  const yamlText = YAML.stringify(obj).trimEnd();
  return `---\n${yamlText}\n---\n${body}`;
}
