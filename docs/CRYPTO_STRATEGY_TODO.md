# Crypto 交易策略 TODO 计划

**日期:** 2026-01-29  
**状态:** 可行性验证完成

---

## 1. 验证结果

### 1.1 Polymarket Crypto 市场现状

| 检查项 | 结果 |
|--------|------|
| 活跃 crypto 价格市场 | ❌ **没有** |
| 历史 crypto 市场 | ✅ 有 (2020年，已关闭) |
| 币安 API | ✅ 可用 |

**原计划不可行原因：** Polymarket 目前没有 "BTC 会不会到 100k?" 这类市场。

### 1.2 可用数据源

```
币安 API (无需 Key，公开数据):
├── GET /api/v3/ticker/price     → 实时价格
├── GET /api/v3/klines           → K线数据
├── GET /api/v3/depth            → 订单簿
└── GET /api/v3/ticker/24hr      → 24h统计
```

---

## 2. 替代方案对比

| 方案 | 可行性 | 预期收益 | 复杂度 | 推荐 |
|------|--------|---------|--------|------|
| A: 直接币安量化交易 | ✅ 高 | 中高 | 高 | ⭐⭐⭐ |
| B: 监控 PM 新市场 | ✅ 高 | 低 | 低 | ⭐⭐ |
| C: 其他预测市场 | 待验证 | 中 | 中 | ⭐⭐ |
| D: Crypto 情绪 → PM 其他市场 | ⚠️ 低 | 低 | 中 | ⭐ |

---

## 3. 方案 A: 币安量化交易 (推荐)

### 3.1 策略设计

既然我们已经有了：
- DeepSeek LLM
- 币安实时数据
- Rust 量化框架

可以直接做 **Crypto 高频/中频交易**。

### 3.2 TODO 执行计划

#### Phase 1: 数据层 (1-2天)
```
□ Task 1.1: 创建 binance_client.rs
  ├── 实时价格获取
  ├── K线数据获取  
  ├── WebSocket 实时行情
  └── 订单簿深度数据

□ Task 1.2: 数据存储
  ├── SQLite 存储历史数据
  └── 内存缓存实时数据
```

#### Phase 2: 信号层 (2-3天)
```
□ Task 2.1: 技术指标计算
  ├── MA/EMA (移动平均)
  ├── RSI (相对强弱)
  ├── MACD
  └── Bollinger Bands

□ Task 2.2: LLM 分析集成
  ├── 新闻情绪分析
  ├── 社交媒体监控 (Twitter/Telegram)
  └── 市场状态判断

□ Task 2.3: 信号生成
  ├── 技术信号 + LLM 信号融合
  ├── 置信度评分
  └── 风险评估
```

#### Phase 3: 执行层 (2-3天)
```
□ Task 3.1: 币安交易接口
  ├── API Key 配置
  ├── 下单接口封装
  ├── 订单状态追踪
  └── 余额管理

□ Task 3.2: 风控系统
  ├── 单笔最大仓位
  ├── 日最大亏损
  ├── 止损止盈
  └── 紧急平仓
```

#### Phase 4: 回测 & 上线 (3-5天)
```
□ Task 4.1: 历史回测
  ├── 获取历史数据
  ├── 模拟交易
  └── 收益分析

□ Task 4.2: Paper Trading
  ├── 模拟盘运行 7天
  └── 调优参数

□ Task 4.3: 实盘上线
  ├── 小资金测试
  └── 逐步加仓
```

### 3.3 代码结构

```
polymarket-bot/
├── src/
│   ├── client/
│   │   ├── binance.rs      ← 新增
│   │   └── ...
│   ├── strategy/
│   │   ├── crypto_hf.rs    ← 新增 (高频策略)
│   │   └── ...
│   ├── indicators/         ← 新增
│   │   ├── ma.rs
│   │   ├── rsi.rs
│   │   └── macd.rs
│   └── ...
```

### 3.4 配置示例

```toml
[binance]
api_key = "${BINANCE_API_KEY}"
api_secret = "${BINANCE_API_SECRET}"

[crypto_strategy]
enabled = true
symbols = ["BTCUSDT", "ETHUSDT", "SOLUSDT"]
timeframe = "5m"
max_position_pct = 0.10    # 单币种最大 10%
stop_loss_pct = 0.02       # 2% 止损
take_profit_pct = 0.05     # 5% 止盈

[crypto_strategy.signals]
rsi_oversold = 30
rsi_overbought = 70
use_llm = true
llm_weight = 0.3           # LLM 信号权重 30%
```

---

## 4. 方案 B: 监控 Polymarket 新市场

### 4.1 逻辑

- 定期扫描 Polymarket API
- 发现新 crypto 市场时立即通知
- 第一时间进入获取 early edge

### 4.2 TODO

```
□ Task B.1: 新市场监控服务
  ├── 每 10 分钟扫描一次
  ├── 检测新市场 (对比历史)
  ├── 筛选 crypto 相关关键词
  └── Telegram 通知

□ Task B.2: 快速响应机制
  ├── 预设分析 prompt
  ├── 自动生成交易信号
  └── 一键下单
```

### 4.3 代码

```rust
// src/monitor/new_market.rs
pub async fn watch_new_markets() {
    let mut known_markets = load_known_markets().await;
    
    loop {
        let current = fetch_all_markets().await;
        
        for market in current {
            if !known_markets.contains(&market.id) {
                // 新市场发现!
                if is_crypto_related(&market.question) {
                    notify_new_crypto_market(&market).await;
                }
                known_markets.insert(market.id);
            }
        }
        
        sleep(Duration::from_secs(600)).await;
    }
}
```

---

## 5. 方案 C: 其他预测市场

### 5.1 待验证平台

| 平台 | 有 Crypto 市场? | API 可用? |
|------|-----------------|-----------|
| Kalshi | 待查 | 待查 |
| Manifold | 待查 | 待查 |
| Metaculus | 待查 | 待查 |
| Augur | 待查 | 待查 |

### 5.2 TODO

```
□ Task C.1: 调研其他预测市场
□ Task C.2: 测试 API 可用性
□ Task C.3: 对比收益机会
```

---

## 6. 立即执行计划

### 今天 (Day 1)

```
[x] 1. 验证 Polymarket crypto 市场 → 结论：没有
[x] 2. 验证币安 API → 可用
[ ] 3. 创建 binance.rs 基础客户端
[ ] 4. 实现价格获取 + K线获取
```

### 明天 (Day 2)

```
[ ] 5. 实现技术指标 (RSI, MA)
[ ] 6. 创建 crypto_hf.rs 策略框架
[ ] 7. 集成到 bot 主循环
```

### Day 3-5

```
[ ] 8. LLM 信号集成
[ ] 9. 回测框架
[ ] 10. Paper trading 测试
```

### Day 6-7

```
[ ] 11. 风控完善
[ ] 12. 实盘小资金测试
```

---

## 7. 风险提示

1. **Crypto 波动大** - 必须严格止损
2. **API 限频** - 币安有请求限制
3. **滑点** - 大单可能滑点严重
4. **市场变化** - 策略需要持续优化

---

## 8. 下一步行动

**请确认你想执行哪个方案：**

- [ ] **方案 A**: 开始做币安量化交易 (推荐)
- [ ] **方案 B**: 先只做 PM 新市场监控
- [ ] **方案 C**: 调研其他预测市场
- [ ] **组合**: A + B 同时进行

确认后我立即开始写代码。
