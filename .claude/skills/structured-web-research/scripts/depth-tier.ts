/**
 * 深度分层算法（纯函数，不依赖 AI 判断）
 *
 * 输入：方向数量
 * 输出：DepthConfig — 嵌套深度、每方向最低来源数、fetch top N
 *
 * 规则完全是确定性的——方向数 → 参数。
 * 不根据"主题复杂度"做任何 heuristics（那是 AI 容易出错的地方）。
 */

interface DepthConfig {
  nestLevel: number;    // 子代理最大嵌套深度
  minSources: number;   // 每方向最低来源数量
  fetchTopN: number;    // 每次 WebSearch 后 fetch 前 N 个结果
  spawnThreshold: number; // 子方向信息量 > 此值才 spawn 孙代理
}

function depthTier(directionCount: number): DepthConfig {
  // 唯一输入：方向数量。完全确定性。
  if (directionCount <= 2) {
    return {
      nestLevel: 1,
      minSources: 3,
      fetchTopN: 3,
      spawnThreshold: Infinity, // 不 spawn 孙代理
    };
  }
  if (directionCount <= 5) {
    return {
      nestLevel: 2,
      minSources: 5,
      fetchTopN: 5,
      spawnThreshold: 8,
    };
  }
  // directionCount > 5
  return {
    nestLevel: 4,
    minSources: 8,
    fetchTopN: 8,
    spawnThreshold: 5,
  };
}

// 单元测试断言（文档用，非可执行）
//
// assert(depthTier(1).nestLevel === 1)
// assert(depthTier(2).fetchTopN === 3)
// assert(depthTier(3).nestLevel === 2)
// assert(depthTier(5).minSources === 5)
// assert(depthTier(6).nestLevel === 4)
// assert(depthTier(10).spawnThreshold === 5)
