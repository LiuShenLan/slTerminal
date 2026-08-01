#!/usr/bin/env node
/**
 * 方向分解（确定性，可执行）
 *
 * 输入：stdin 读 concepts JSON，如 [{"name":"Claude Code hooks","facets":["配置","集成"]}]
 *   - name: 概念名
 *   - facets: 该概念的特殊关注面（可为空数组）
 *   - facetSlugs（可选）: 与 facets 一一对应的英文 kebab-case slug 数组
 *     存在且长度匹配时覆盖自动生成的中文 slug；否则回退现有规则
 * 输出：stdout JSON Direction[]，每个方向 { title, nestable, outputSlug }
 *
 * 确定性规则：
 *   - facets 非空 → 每个 facet 一个方向（nestable=true）
 *   - facets 为空 → 概念 × 4 维矩阵（RESEARCH_DIMENSIONS）
 *   - 方向总数 > 8 → 合并非 nestable 方向至总数 6
 *
 * 搜索关键词不在此生成——由主代理对输出方向逐个生成（语义工作）。
 *
 * 用法：echo '[{"name":"X","facets":[],"facetSlugs":[]}]' | node decompose.mjs
 */

/** 维度矩阵——确定性，对所有主题不变 */
const RESEARCH_DIMENSIONS = [
  { key: "official",   label: "官方文档/规范",          nestable: false },
  { key: "community",  label: "社区讨论/GitHub issues", nestable: true },
  { key: "comparison", label: "同类工具/项目的对比参考", nestable: true },
  { key: "cases",      label: "实际使用案例/配置示例",   nestable: true },
];

/** kebab-case 转换，截断至 60 字符（防长概念名产生超长文件名） */
function slugify(text) {
  const slug = text.toLowerCase().replace(/\s+/g, "-");
  return slug.length <= 60 ? slug : slug.slice(0, 60).replace(/-+$/, "");
}

/** 概念 → 方向列表（纯函数） */
function decompose(concepts) {
  const directions = [];

  for (const concept of concepts) {
    if (Array.isArray(concept.facets) && concept.facets.length > 0) {
      // 有关注面：每 facet 独立方向，可深入
      // facetSlugs 与 facets 一一对应时优先使用（英文 kebab-case），否则回退自动生成
      const slugs =
        Array.isArray(concept.facetSlugs) && concept.facetSlugs.length === concept.facets.length
          ? concept.facetSlugs
          : null;
      for (let i = 0; i < concept.facets.length; i++) {
        const facet = concept.facets[i];
        directions.push({
          title: `${concept.name} — ${facet}`,
          nestable: true,
          outputSlug: slugs ? slugs[i] : slugify(`${concept.name}-${facet}`),
        });
      }
    } else {
      // 无关注面：概念 × 维度矩阵
      for (const dim of RESEARCH_DIMENSIONS) {
        directions.push({
          title: `${concept.name} — ${dim.label}`,
          nestable: dim.nestable,
          outputSlug: slugify(`${concept.name}-${dim.key}`),
        });
      }
    }
  }

  // 确定性合并：超过 8 个方向 → 合并非 nestable 方向至总数 6
  if (directions.length > 8) {
    return mergeDirections(directions, 6);
  }
  return directions;
}

/** 合并非 nestable 方向（nestable 保持独立，可并行深入） */
function mergeDirections(dirs, target) {
  const nonNestable = dirs.filter((d) => !d.nestable);
  const nestable = dirs.filter((d) => d.nestable);

  while (nonNestable.length + nestable.length > target && nonNestable.length > 1) {
    const a = nonNestable.pop();
    const b = nonNestable.pop();
    nonNestable.push({
      title: `${a.title} + ${b.title.split("—")[1]?.trim() ?? b.title}`,
      nestable: false,
      outputSlug: `${a.outputSlug}-${b.outputSlug}`,
    });
  }

  return [...nonNestable, ...nestable];
}

// 读取 stdin
let input = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => (input += chunk));
process.stdin.on("end", () => {
  let concepts;
  try {
    concepts = JSON.parse(input);
  } catch {
    console.error('输入不是合法 JSON。期望: [{"name":"概念","facets":["关注面"]}]');
    process.exit(1);
  }
  if (!Array.isArray(concepts) || concepts.length === 0 || concepts.some((c) => !c.name)) {
    console.error('输入须为非空数组，元素含 name 字段: [{"name":"概念","facets":[]}]');
    process.exit(1);
  }
  process.stdout.write(JSON.stringify(decompose(concepts), null, 2) + "\n");
});
