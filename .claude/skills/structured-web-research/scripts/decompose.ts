/**
 * 方向分解算法（伪代码）
 *
 * 输入：AI 提取的核心概念列表 + 用户确认的范围
 * 输出：Direction[] — 每个方向含标题、搜索关键词、是否可嵌套
 *
 * 此函数是确定性的——同样的输入永远产生同样的方向结构。
 * AI 仅负责"提取核心概念"这一步（需要语义理解）。
 */

interface Concept {
  name: string;        // 概念名，如 "Claude Code hooks"
  facets: string[];    // 该概念的特殊关注面，如 ["配置", "可视化", "集成"]
}

interface Direction {
  title: string;       // 方向标题
  keywords: string[];  // 搜索关键词（由 AI 填充）
  nestable: boolean;   // 是否允许 spawn 孙代理
  outputSlug: string;  // D1-{slug}.md 中的 {slug}
}

// 维度矩阵——确定性，对所有主题不变
const RESEARCH_DIMENSIONS = [
  { key: "official",   label: "官方文档/规范",         nestable: false },
  { key: "community",  label: "社区讨论/GitHub issues", nestable: true  },
  { key: "comparison", label: "同类工具/项目的对比参考", nestable: true  },
  { key: "cases",      label: "实际使用案例/配置示例",  nestable: true  },
] as const;

function decompose(concepts: Concept[]): Direction[] {
  const directions: Direction[] = [];

  for (const concept of concepts) {
    for (const dim of RESEARCH_DIMENSIONS) {
      // 特殊维度独立成方向
      if (concept.facets.length > 0) {
        for (const facet of concept.facets) {
          directions.push({
            title: `${concept.name} — ${facet}`,
            keywords: [], // AI 填充
            nestable: dim.nestable,
            outputSlug: `${concept.name}-${facet}`.toLowerCase().replace(/\s+/g, "-"),
          });
        }
      } else {
        directions.push({
          title: `${concept.name} — ${dim.label}`,
          keywords: [],
          nestable: dim.nestable,
          outputSlug: `${concept.name}-${dim.key}`.toLowerCase(),
        });
      }
    }
  }

  // 确定性合并：超过 8 个方向 → 合并同类至 5-8 个
  if (directions.length > 8) {
    return mergeDirections(directions, 6);
  }

  return directions;
}

function mergeDirections(dirs: Direction[], target: number): Direction[] {
  // 按 nestable 分组，优先合并非 nestable 的方向
  const nonNestable = dirs.filter(d => !d.nestable);
  const nestable = dirs.filter(d => d.nestable);

  // 保持 nestable 独立（可并行深入），合并非 nestable
  while (nonNestable.length + nestable.length > target && nonNestable.length > 1) {
    const a = nonNestable.pop()!;
    const b = nonNestable.pop()!;
    nonNestable.push({
      title: `${a.title} + ${b.title.split("—")[1]?.trim() ?? b.title}`,
      keywords: [],
      nestable: false,
      outputSlug: `${a.outputSlug}-${b.outputSlug}`,
    });
  }

  return [...nonNestable, ...nestable];
}
