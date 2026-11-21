∆, [21.11.2026 05:36]
Worked for 1m 56s

### Fieldbook: technical review and comparison with prop-firm/HFT systems

My initial assessment: Fieldbook is a market-data infrastructure project, not a trading system. Its intended role is to collect, normalize, capture and preserve exchange market data. It does not currently implement the trading, strategy execution, portfolio, or risk-management layers that make up a professional proprietary-trading platform.

That distinction matters. A good market-data collector can be a useful component of a quant trading stack, but it is not equivalent to the stack itself.

The repository describes a Rust-based, read-only collector covering Kalshi, Polymarket and Hyperliquid, with REST discovery, WebSocket ingestion, FIX SBE capture and checksummed raw-frame journaling. It explicitly excludes order placement, positions, strategy execution and pre-trade risk. GitHub

### 1\. What is actually implemented?

I reviewed the architecture documentation, Rust modules, exchange adapters, data models, journal, writer, analytics pipeline, requirement-to-test matrix and programme-status documents.

The actual system looks like this:

flowchart TD
    A[Exchange REST APIs] --> B[Market discovery]
    B --> C[Capacity planner]
    C --> D[WebSocket session workers]
    D --> E[Protocol decoding]
    E --> F[Normalized market events]
    F --> G[CollectorCore]
    G --> H[Order book state]
    G --> I[SBE writer]
    G --> J[Raw-frame journal]
    G --> K[Prometheus metrics]
    I --> L[Binary market-data capture]
    J --> M[Checksummed journal]
    L --> N[Verification and replay]
    M --> N
    N --> O[Parquet export]
    O --> P[KDB-X / q analytics]
This is a sensible architecture for collecting and validating market data.

### Main components

| Component               | What it does                                      | My assessment                      |
| ----------------------- | ------------------------------------------------- | ---------------------------------- |
| REST discovery          | Discovers markets and instruments                 | Useful                             |
| WebSocket ingestion     | Receives exchange updates                         | Core functionality                 |
| Exchange adapters       | Kalshi, Polymarket, Hyperliquid                   | Uneven maturity                    |
| CollectorCore           | Maintains normalized order-book state             | One of the stronger components     |
| Recovery                | Replicas, failover, snapshots and recovery fences | Thoughtfully designed              |
| SBE writer              | Writes normalized market data                     | Good separation                    |
| Raw-frame journal       | Preserves original incoming messages              | Particularly valuable              |
| Replay and verification | Reconstructs and checks captured data             | Strong engineering direction       |
| Prometheus              | Health, readiness and operational metrics         | Useful                             |
| Analytics               | Parquet export and KDB-X querying                 | Early research-data infrastructure |

The most important architectural decision is keeping exchange connectivity, the normalized data model, state management, writing and journaling separate.

The coordinator supervises session workers, while a synchronous CollectorCore manages the books. A separate writer thread serializes SBE output. The journal has its own independent persistence path. GitHub +2

This is considerably more thoughtful than a typical exchange WebSocket application.

### 2\. What I particularly like in the implementation

There are several genuinely good engineering decisions.

A. Exact numerical representation

The project uses integer-based Price and Quantity types, with instrument-specific numerical specifications.

This avoids the floating-point precision errors that frequently corrupt financial calculations.

∆, [21.11.2026 05:36]
It also handles instruments with different price scales, tick sizes, multipliers and quantity units. The general numerical model is a good foundation for supporting markets beyond binary prediction contracts. GitHub +1

B. Recovery is treated as a correctness problem

This is probably the strongest part of the architecture.

It distinguishes:

* Connection health.

* Instrument activity.

* Order-book continuity.

These are different things.

For example, a quiet instrument should not automatically cause an entire exchange connection to reconnect.

The implementation supports shadow books for replicated sources, explicit recovery acknowledgements, fresh snapshot anchoring and bounded recovery operations.

That is the right way to think about market-data reliability. GitHub +2

C. Raw-frame journaling

This is arguably Fieldbook's most valuable feature.

It preserves the exact bytes received from the exchange, including messages rejected by the decoder.

The journal supports:

* Append-only records.

* SHA-256 checksums.

* Versioned segments.

* Record and segment verification.

* Recovery of incomplete captures.

* Explicit durability states.

* Bounded backpressure.

One particularly important detail: the implementation distinguishes a record being written from it being flushed, synced and finalized.

That distinction matters when reconstructing data after a process crash. GitHub +1

D. Deterministic testing

The market-data core can be exercised independently of live network connections.

There are tests for dropped updates, stale subscriptions, reconnects, recovery races, book reconstruction, journal corruption and shutdown failures.

The requirement-to-test matrix also maps individual engineering requirements to actual test functions, rather than simply saying that the application has tests. GitHub +1

These are good signs of engineering maturity.

### 3\. The critical limitations

This is where Fieldbook diverges substantially from established quantitative trading infrastructure.

### A. Its order book is not an HFT-grade order book

Inspect src/orderbook.rs.

The implementation uses:

pub struct OrderBook {
    bids: BTreeMap<Price, Quantity>,
    asks: BTreeMap<Price, Quantity>,
}
This is an aggregated price-level book.

It maintains the total quantity available at each bid and ask price.

It does not maintain the individual identity and lifecycle of every resting order.

That distinction is enormous.

Consider:

| Capability                          | Aggregated L2     | Order-level L3 |
| ----------------------------------- | ----------------- | -------------- |
| Best bid/ask                        | Yes               | Yes            |
| Quantity at each price              | Yes               | Yes            |
| Individual order ID                 | No                | Yes            |
| Individual order additions          | Aggregated effect | Yes            |
| Individual cancellations            | Aggregated effect | Yes            |
| Queue-position estimation           | Very limited      | Possible       |
| Order-level microstructure analysis | Limited           | Much stronger  |

Nasdaq TotalView, for example, provides every displayed quote and order at each price level. Databento's MBO format preserves individual order events and order IDs, enabling queue-position analysis. Nasdaq Insights +1

Why this matters:

Suppose you want to build a market-making strategy.

You need to understand:

* How much volume is ahead of your order.

* How quickly the queue is changing.

* Whether orders are being cancelled or executed.

* The likelihood of your order getting filled.

* Whether aggressive participants are consuming liquidity.

Fieldbook's aggregated book cannot independently answer all these questions.

This is not a minor missing feature. It changes what strategies you can realistically research.

### B. The Hyperliquid implementation is narrower than it initially appears

Inspecting src/exchange/hyperliquid/session.rs reveals something important.

The adapter subscribes to:

∆, [21.11.2026 05:36]
* allMids

* fastAssetCtxs

* l2Book

* trades

However, the implementation treats the first two as telemetry.

The selected l2Book messages become normalized book snapshots. Trades are retained as telemetry rather than applied as book deltas.

There is currently no LevelChange generation from that adapter.

The selected universe is finite, with a default of ten perpetuals, and is not a complete exchange-wide market-data implementation. GitHub +2

That means you should not assume Fieldbook is maintaining a continuously updated, granular, order-by-order representation of the entire Hyperliquid market.

### C. It has no demonstrated HFT latency advantage

This is the biggest difference between an ordinary quant data application and HFT infrastructure.

Fieldbook uses Rust, Tokio, WebSockets, HTTP REST and asynchronous session workers.

These are reasonable technologies for a market-data collector.

But there is no publicly demonstrated:

* Kernel-bypass networking.

* FPGA-based packet capture.

* Exchange-colocated feed handling.

* PTP-synchronized hardware timestamps.

* Zero-copy high-throughput market-data distribution.

* Hardware-counter-driven optimization.

* Validated microsecond or sub-microsecond processing performance.

The distinction is not that Rust cannot be used for HFT. Rust can be used for extremely fast systems.

The distinction is that Fieldbook does not demonstrate the infrastructure, measurements or operating environment necessary to establish HFT performance.

Its own performance methodology explicitly states that the timing histograms are not a production benchmark or certification. It does not certify p50, p95, p99 or p99.9 latency targets, throughput, CPU consumption or cross-host performance. GitHub +1

### 4\. Comparison against established HFT engineering

There is an important qualification here: firms such as Jane Street, Citadel Securities and Hudson River Trading do not publish their complete proprietary trading architectures.

I therefore use their public engineering requirements, exchange specifications and specialist market-data infrastructure as comparison points, rather than pretending to know their confidential implementations.

### Market-data infrastructure

Consider three useful benchmarks.

Jane Street

Its public low-latency engineering requirements specifically mention:

* Low-level systems programming.

* High-throughput network applications.

* Hardware performance counters.

* CPU cache architecture.

* Fast packet processing in user space.

* Kernel-bypass technologies such as DPDK and Solarflare. janestreet.com +1

CME Group

CME's MDP 3.0 infrastructure uses redundant UDP multicast feeds, sequence numbers, market recovery and TCP recovery.

CME strongly recommends processing both incremental feeds and requires clients to certify their market recovery functionality.

This is a very different operational environment from consuming ordinary public WebSocket streams. Confluence +1

Databento

Its published market-data architecture describes colocation, FPGA-based packet capture, redundant feed arbitration, zero-copy messaging, SmartNIC-based networking and real-time normalization.

It reports approximately 5–35 microseconds of processing inside its capture servers, depending on the venue. These are Databento's own published figures, not independently verified comparisons with Fieldbook. Databento Docs +1

### Direct comparison

| Capability            | Fieldbook                                 | Established HFT infrastructure benchmark                       |
| --------------------- | ----------------------------------------- | -------------------------------------------------------------- |
| Market connectivity   | REST and WebSockets                       | Direct feeds and specialized connectivity                      |

∆, [21.11.2026 05:36]
| Network architecture  | Standard software networking              | Colocation, multicast, specialized packet processing           |
| Feed redundancy       | Replicated WebSocket connections          | Multiple redundant feed paths and packet arbitration           |
| Book representation   | Aggregated price-level book               | Often full order-level data where available                    |
| Sequence and recovery | Explicit application-level recovery logic | Exchange-specific packet sequencing and recovery certification |
| Timestamps            | Receive timestamps and software timing    | Precisely synchronized timestamps, often hardware assisted     |
| Latency optimization  | Some instrumentation                      | Aggressive hardware and software optimization                  |
| Strategy engine       | Absent                                    | Integrated with internal strategy systems                      |
| Execution             | Absent                                    | Order gateways and execution systems                           |
| Risk management       | Absent                                    | Dedicated real-time risk controls                              |
| Production validation | Partial evidence                          | Operational and performance certification                      |

My assessment: Fieldbook is several architectural layers below a mature HFT platform.

However, its recovery, data validation and journaling concepts are legitimate building blocks for one.

### 5\. Comparison against a quant research platform

This is a slightly different comparison.

A quantitative research platform does not necessarily need nanosecond execution.

It needs high-quality historical data, research tooling, reproducibility, statistical analysis and backtesting.

Fieldbook is closer to this category, although it is not yet a complete quant research platform either.

### What it has

The analytics implementation is more interesting than the README initially suggests.

I inspected src/analytics.rs and the requirement-to-test matrix.

The pipeline includes:

* Independent verification of captured market data.

* SHA-256 verification of source artifacts.

* Export of journal observations and SBE messages.

* Typed intermediate datasets.

* Parquet generation with schema validation.

* KDB-X virtual-table queries.

* Rebuildable, partitioned HDB datasets.

The project reports successful reconciliation of three retained exports across 12 checks. GitHub +2

This is useful.

KDB-X is also a relevant technology to examine because time-series querying and historical market-data analysis are important parts of quantitative trading systems.

But Fieldbook's analytics pipeline primarily verifies and organizes captured data.

It does not currently provide a complete research environment.

### What is missing

| Quant research capability          | Fieldbook                             |
| ---------------------------------- | ------------------------------------- |
| Historical tick-data management    | Partial                               |
| Full historical order-book archive | Partial                               |
| Order-flow analytics               | Limited                               |
| Microstructure features            | Not developed as a research framework |
| Alpha-factor library               | Absent                                |
| Statistical research notebooks     | Not a core application                |
| Event-driven backtesting           | Absent                                |
| Execution simulation               | Absent                                |
| Transaction-cost modelling         | Absent                                |
| Market-impact modelling            | Absent                                |
| Portfolio optimization             | Absent                                |
| Strategy performance evaluation    | Absent                                |
| Model training and deployment      | Absent                                |

∆, [21.11.2026 05:36]
An important distinction: deterministic replay in Fieldbook means replaying recorded market-data events through its collector. It does not mean replaying a market with simulated orders, fills, queue priority and market impact.

Those are different engineering problems.

### What a more complete quant research stack looks like

flowchart TD
    A[Exchange market data] --> B[Market data capture]
    B --> C[Historical tick database]
    C --> D[Research and feature engineering]
    D --> E[Alpha models]
    E --> F[Backtesting and simulation]
    F --> G[Strategy validation]
    G --> H[Portfolio construction]
    H --> I[Execution algorithms]
    I --> J[Broker or exchange gateways]
    J --> K[Live positions and P&L]
    K --> L[Risk and monitoring]
Fieldbook is mostly concentrated around B and part of C.

It has some of the engineering foundations needed to build the rest, but the rest is not implemented.

### 6\. An important finding: current evidence and production readiness

I would pay particular attention to this section before deploying the software.

### Exchange integration status

Based on the programme-status document:

| Venue       | Implementation status                     | Live evidence                                                  |
| ----------- | ----------------------------------------- | -------------------------------------------------------------- |
| Hyperliquid | H0–H2 implemented                         | Operator-reported mainnet captures                             |
| Polymarket  | Adapter exists                            | Live connections currently disabled; historical failed capture |
| Kalshi      | Adapter and authenticated read-only tests | No live capture claimed                                        |
| Coinbase    | H0 reference data                         | No live feed                                                   |
| Kraken      | H0 reference data                         | No live feed                                                   |
| OKX         | H0 reference data                         | No live feed                                                   |
| Binance     | H0 reference data                         | No live feed                                                   |

GitHub +2

There is a documentation inconsistency worth flagging: the programme-status document reports five Hyperliquid mainnet captures, but the requirement-test matrix still contains language stating that Hyperliquid live validation has not been established.

The programme-status document declares itself authoritative, but the capture artifacts are excluded from the public source export. I would therefore classify the live claims as operator-reported rather than independently verified. GitHub +1

### The Polymarket data-quality failure

This is particularly revealing.

The README documents a 60-minute full-universe capture in which:

* The writer had zero errors.

* 756 WebSocket errors occurred.

* Approximately ten million failover snapshots were generated.

* 12.34% of snapshots failed SBE reconstruction.

This is a good example of why zero writer errors cannot be treated as proof of correct market data.

The software successfully wrote data, but a significant percentage of the recorded market state failed reconstruction. The project correctly preserves this as failed evidence rather than calling it a successful certification. GitHub +1

### Capacity claims need careful interpretation

The operating-envelope document reports a synthetic 100,000-symbol test using an in-process mock exchange.

However, the programme-status document subsequently identifies only a 10,000-symbol short capacity cell as executed and explicitly makes no capacity certification.

It also states that 25,000 and 50,000-symbol tests were not executed.

Consequently, I would not treat the 100,000-symbol number as evidence of live exchange-feed capacity. GitHub +1

### Production-readiness gaps

The operating-envelope document explicitly lists several unresolved items:

∆, [21.11.2026 05:36]
| Requirement                  | Documented status                  |
| ---------------------------- | ---------------------------------- |
| 24-hour uninterrupted soak   | Not executed                       |
| 24-hour fault-injection soak | Not executed                       |
| Security review              | Not executed                       |
| Deployment review            | Not executed                       |
| Exact-commit CI/release gate | Listed as pending in that document |

GitHub +1

These are significant gaps for a system that would be entrusted with trading-critical data.

### 7\. My overall evaluation

Here is how I would classify Fieldbook against different potential applications.

| Intended application                         | Suitability     | Verdict                                               |
| -------------------------------------------- | --------------- | ----------------------------------------------------- |
| Exchange data collector                      | Good foundation | Worth developing                                      |
| Data integrity and capture verification      | Strong          | Particularly promising                                |
| Prediction-market market-data infrastructure | Reasonable      | Appropriate initial scope                             |
| Quant research data ingestion                | Partial         | Useful with additional infrastructure                 |
| Quantitative backtesting platform            | Insufficient    | Major development required                            |
| Market-making research platform              | Insufficient    | Needs MBO, trades and execution simulation            |
| Automated trading application                | Not suitable    | No trading engine                                     |
| Production proprietary trading platform      | Not suitable    | Missing critical subsystems                           |
| Low-latency HFT platform                     | Not suitable    | No demonstrated HFT networking or latency performance |

These are my qualitative assessments, not measured benchmark scores.

### 8\. What I would build next to make it competitive

I would not throw away Fieldbook. Its existing architecture has enough merit to justify building on it, especially its journaling, validation and recovery framework.

But I would significantly expand its scope.

My recommended development priorities would be:

Priority 1 — Institutional-grade market data

Move beyond public WebSocket feeds where direct feeds are available.

Add:

* Native exchange feed adapters.

* Market-by-order support.

* Exchange sequencing and packet-gap recovery.

* Precise event and receive timestamps.

* Redundant feed arbitration.

* Full historical capture and replay.

Priority 2 — Quant research infrastructure

Build:

* A reliable historical tick database.

* Order-flow analytics.

* Trade and order-event reconstruction.

* Queue-position estimation.

* Microstructure feature engineering.

* Statistical research tooling.

The existing Parquet/KDB-X work provides a useful starting point.

Priority 3 — Backtesting

Develop a proper event-driven backtester with:

* Order-book simulation.

* Market and limit orders.

* Partial fills.

* Queue priority.

* Fees and rebates.

* Latency simulation.

* Slippage and market impact.

Without realistic fills, a strategy can appear profitable in research and fail immediately in live trading.

Priority 4 — Trading infrastructure

Separate the trading system from the market-data collector.

Build:

* Strategy runtime.

* Order management system.

* Execution engine.

* Exchange gateways.

* Pre-trade risk.

* Position and exposure management.

* Emergency kill switches.

* Trade reconciliation.

Priority 5 — HFT performance engineering

Only after correctness is established, optimize:

* Thread and CPU affinity.

* Memory allocation.

* Lock contention.

* Cache locality.

* Network-stack overhead.

* Lock-free message passing where justified.

* Tail latency.

* Hardware-assisted packet processing.

∆, [21.11.2026 05:36]
Measure p50, p99 and p99.9 under realistic peak market-data rates, rather than relying on synthetic throughput demonstrations.

### 9\. Final verdict

I would describe Fieldbook as:

A promising, technically thoughtful market-data collection, recovery and data-integrity project with early quantitative research infrastructure. It is not yet a quantitative trading platform, and it is nowhere near a production HFT platform.

Its best features are not its trading capabilities. It has none.

Its strengths are its data provenance, exact numerical representation, recovery state machine, deterministic testing and raw market-data journaling.

Those are genuinely valuable capabilities.

But established proprietary trading firms compete on much more than collecting exchange prices. They operate integrated environments spanning market data, microstructure, research, execution, risk and low-latency infrastructure.

Fieldbook currently addresses only a fraction of that problem.

My recommendation: treat it as a potentially useful market-data subsystem, not as the foundation of a finished trading business. Validate its data correctness and live operational performance before investing heavily in the higher-level strategy and execution layers.

### Review scope and confidence

I inspected the repository's public source and architecture, including collector/state.rs, orderbook.rs, writer.rs, journal.rs, coordinator.rs, the market-data models, exchange sessions, analytics implementation, Cargo manifest and the associated engineering documentation.

This was a static source review. I did not execute the test suite or independently reproduce live exchange captures. The operator-local capture artifacts are not included in the public source export, which limits independent verification.

One particularly useful next step would be a deeper architecture gap analysis against a Jane Street/Citadel-style trading stack, mapping each missing subsystem to concrete Rust services, interfaces and development priorities.

∆, [21.11.2026 05:42]
Worked for 2m 5s

### The vision: Build a global financial information collection infrastructure

I would take Fieldbook in a fundamentally bigger direction.

I would not position it as a crypto market-data collector, prediction-market collector, or even simply an exchange-data collector.

I would position it as:

A universal financial and economic data acquisition platform covering the world's financial markets, instruments, entities, transactions, prices, reference information and market events—with complete source provenance, historical preservation, normalization and access.

Think of combining aspects of:

* Interactive Brokers for breadth of markets and instruments.

* Bloomberg for breadth of financial information.

* LSEG for global reference data, pricing and historical information.

* ICE Data Services for multi-asset market data.

* Databento for exchange-level market-data capture and historical tick data.

But with one important distinction:

Fieldbook should not become a broker, trading terminal, execution platform or investment-management application. It should become the infrastructure that collects and makes financial information available to everyone who is authorized to consume it.

The closest commercial benchmark is not just IBKR. It is the global data infrastructure provided by Bloomberg and LSEG, combined with the depth and technical rigor of specialist exchange-data infrastructure.

For perspective, LSEG publicly describes a portfolio spanning more than 100 million instruments, over 500 venues and contributors, and 80 million securities in its reference-data coverage. These are the kinds of scale indicators against which your ambition should eventually be measured. Data Analytics +1

### 1\. First, establish what "everything" means

I would make a distinction between five dimensions of completeness.

| Dimension            | What Fieldbook must collect                                                  |
| -------------------- | ---------------------------------------------------------------------------- |
| Geographic           | 90+ countries and their relevant markets                                     |
| Asset coverage       | Every major financial asset class                                            |
| Instrument coverage  | Every supported instrument, contract and listing                             |
| Information coverage | Every available data category associated with those instruments and entities |
| Temporal coverage    | Historical, current, real-time, delayed, corrected and revised data          |

There is a sixth dimension that I consider mandatory:

Source completeness. Fieldbook must know which source provides each piece of information, what that source actually covers, what it omits, how reliable it is and whether it is licensed for the intended use.

A country being marked as supported because you collect one exchange's equity prices is unacceptable.

For example, support for Japan should be evaluated independently across equities, government bonds, corporate bonds, futures, options, FX, funds, economic indicators, company information and other relevant data.

One country can have dozens of distinct data sources and hundreds of different information products.

I would build a coverage matrix:

Country × Venue × Asset class × Instrument × Data category × Depth × Frequency × History × Source × Rights

This becomes the foundation of the entire project.

### 2\. The complete universe of information Fieldbook should collect

This is the most important architectural decision.

Do not design Fieldbook around exchange names.

Design it around the universal universe of financial information, with exchange and other data-source integrations feeding that universe.

### A. Equities and listed securities

Support global exchanges, including developed, emerging and frontier markets.

Instruments:

* Common shares.

* Preferred shares.

* Depositary receipts (ADR/GDR).

* ETFs and ETNs.

* REITs.

* Mutual funds and exchange-listed funds.

* Closed-end funds.

* Warrants.

* Rights.

∆, [21.11.2026 05:42]
* Convertible securities.

* SPACs.

* IPOs.

* Delisted and suspended securities.

* OTC securities where data is legally and commercially accessible.

Information to collect:

* Level 1 quotes.

* Level 2 market depth.

* Level 3 order-by-order messages where offered.

* Full trade prints.

* Order additions, modifications and cancellations.

* Auction information.

* Opening and closing prices.

* Trading halts.

* Circuit breakers.

* Market status.

* Volume and turnover.

* VWAP.

* Imbalances.

* Short-sale information where available.

* Corporate actions.

* Dividends.

* Splits.

* Rights issues.

* Share buybacks.

* Historical prices.

* Company fundamentals.

* Earnings.

* Analyst estimates.

* Insider and institutional ownership where available.

A good benchmark here is Nasdaq TotalView, which provides order-level information rather than just aggregated price levels. Nasdaq Insights +1

### B. Fixed income

This is an area where I would invest heavily.

Many data collection systems have excellent equity support and comparatively weak fixed-income coverage.

Fieldbook should support:

Government debt

* Treasury bills.

* Government bonds.

* Inflation-linked bonds.

* Sovereign debt.

* Municipal bonds.

* Treasury auctions.

Corporate debt

* Investment-grade bonds.

* High-yield bonds.

* Convertible bonds.

* Perpetual bonds.

* Callable and puttable bonds.

* Private placements, where available.

Structured products

* Mortgage-backed securities.

* Asset-backed securities.

* Collateralized loan obligations.

* Collateralized debt obligations.

Money markets

* Commercial paper.

* Certificates of deposit.

* Treasury bills.

* Repurchase agreements.

* Reverse repos.

* Interbank rates.

* Overnight funding rates.

Collect:

* Dealer quotes.

* Executed trades.

* Evaluated pricing.

* Bid and ask yields.

* Clean and dirty prices.

* Accrued interest.

* Duration.

* Convexity.

* Spreads.

* Credit ratings.

* Coupon schedules.

* Maturities.

* Call schedules.

* Yield curves.

* Benchmark curves.

* Issuance and auction results.

Important: fixed income is not one uniform order-book market. Some instruments are traded through dealer networks, RFQs and bilateral transactions. Fieldbook must support quote observations and evaluated prices without pretending they are exchange order books.

### C. Futures and options

Support every major exchange and contract family where data rights and availability permit.

Futures

* Equity index futures.

* Individual equity futures.

* Interest-rate futures.

* Government bond futures.

* Currency futures.

* Commodity futures.

* Energy futures.

* Agricultural futures.

* Freight futures.

* Carbon futures.

* Cryptocurrency futures.

Options

* Equity options.

* Index options.

* ETF options.

* Currency options.

* Interest-rate options.

* Commodity options.

* Options on futures.

* Volatility options.

* Cryptocurrency options.

Information:

* Full contract specifications.

* Expiry and settlement rules.

* Strike prices.

* Contract multipliers.

* Exercise styles.

* Deliverables.

* Settlement methods.

* Open interest.

* Volume.

* Bid/ask.

* Order-book depth.

* Implied volatility.

* Greeks.

* Volatility surfaces.

* Skew and term structure.

* Expiration calendars.

* Exercise and assignment information where available.

You must also collect the underlying index and reference data.

An option without accurate contract specifications, corporate-action adjustments and an associated underlying-instrument identity is incomplete data.

### D. Foreign exchange

This needs to cover the global FX market, not just a few retail currency APIs.

Collect:

* Major currency pairs.

* Minor pairs.

* Exotic pairs.

* Spot.

* Forwards.

* NDFs.

* FX swaps.

* Currency options.

* Interbank quotes.

* ECN quotes.

* Available transaction reports.

* Central-bank fixings.

* Settlement data.

* FX forward curves.

* Cross-currency basis.

* Currency funding and interest-rate differentials.

∆, [21.11.2026 05:42]
One important limitation: FX is fragmented and predominantly OTC. There is no single universal consolidated global FX order book.

Therefore, Fieldbook must preserve venue, contributor and source identity instead of creating a misleading universal price.

### E. Commodities and physical markets

This must go substantially beyond futures prices.

Energy

* Crude oil.

* Refined petroleum.

* Natural gas.

* LNG.

* Electricity.

* Coal.

* Renewable energy certificates.

Metals

* Gold.

* Silver.

* Platinum.

* Palladium.

* Copper.

* Aluminium.

* Nickel.

* Zinc.

* Steel.

* Iron ore.

Agriculture

* Wheat.

* Corn.

* Soybeans.

* Rice.

* Sugar.

* Coffee.

* Cocoa.

* Cotton.

* Livestock.

Other markets

* Carbon credits.

* Emission allowances.

* Freight.

* Shipping.

* Physical commodity assessments.

* Agricultural production.

* Storage and inventory.

Information includes:

* Spot prices.

* Futures curves.

* Physical assessments.

* Basis.

* Location differentials.

* Supply and demand.

* Inventory.

* Production.

* Imports and exports.

* Weather.

* Shipping and transport.

* Delivery specifications.

Physical commodity information and assessments must have their own data models. They should not be forced into the same model as an exchange-traded equity.

### F. Digital assets

Retain and improve the existing Fieldbook work.

Cover:

* Spot exchanges.

* Perpetual swaps.

* Futures.

* Options.

* Tokenized securities.

* Stablecoins.

* DeFi protocols.

* Decentralized exchanges.

* On-chain transactions.

* Lending protocols.

* Liquid staking.

* Validators.

* Blockchain network activity.

* Token supply.

* Token unlocks.

* Protocol treasury data.

And importantly:

* Wallet-level on-chain information.

* Exchange-level order books.

* Trade execution data.

* Funding rates.

* Liquidations.

* Open interest.

* Borrowing and lending rates.

* Pool liquidity.

* Token holder distributions.

Keep on-chain observations distinct from exchange-provided data. Their provenance and semantics are different.

### G. Indices, benchmarks and reference rates

This deserves its own data domain.

* Equity indices.

* Sector indices.

* Bond indices.

* Commodity indices.

* Volatility indices.

* FX benchmarks.

* Interest-rate benchmarks.

* Credit indices.

* Inflation indices.

* Economic benchmarks.

* Fund benchmarks.

Capture index composition, constituent changes, weight changes, methodology versions, rebalance information and historical revisions wherever available.

### H. Corporate, issuer and financial information

This is where Fieldbook can evolve beyond a conventional exchange-feed collector.

Collect:

* Financial statements.

* Balance sheets.

* Income statements.

* Cash-flow statements.

* Earnings releases.

* Annual reports.

* Quarterly reports.

* Regulatory filings.

* Shareholder disclosures.

* Insider transactions.

* Institutional ownership.

* Company announcements.

* Mergers and acquisitions.

* IPO filings.

* Bankruptcy filings.

* Credit ratings.

* Credit rating changes.

* Dividends and distributions.

* Capital changes.

* Corporate actions.

* Company forecasts.

* Analyst estimates.

* Management guidance.

Keep both the original document and structured extracted facts, with references to the original source and publication time.

### I. Macroeconomic and government data

Every major country should have a dedicated economic-data catalogue.

Collect:

* GDP.

* CPI.

* PPI.

* Inflation expectations.

* Employment.

* Unemployment.

* Industrial production.

* PMI.

* Retail sales.

* Trade balance.

* Imports and exports.

* Government debt.

* Fiscal deficits.

* Central-bank rates.

* Monetary-policy announcements.

* Government bond issuance.

* Foreign reserves.

* Balance of payments.

* Housing data.

* Consumer confidence.

* Business sentiment.

* Economic forecasts.

Also capture:

* Original releases.

* Release calendars.

* Preliminary values.

* Revised values.

* Seasonal adjustments.

* Revision histories.

* Forecast consensus, where licensed.

∆, [21.11.2026 05:42]
Never overwrite an original macroeconomic release when the agency later revises it.

### J. News, events and alternative information

For a truly comprehensive platform, include:

* Global financial news.

* Exchange notices.

* Central-bank announcements.

* Regulatory notices.

* Earnings calendars.

* Economic calendars.

* Company announcements.

* Geopolitical events.

* Government announcements.

* Weather observations and forecasts.

* Shipping movements.

* Vessel information.

* Satellite-derived commercial indicators.

* Supply-chain information.

* Web and document intelligence.

* Licensed sentiment information.

* Research reports.

Alternative data needs additional privacy, consent, intellectual-property and source-rights controls.

### 3\. The architecture Fieldbook needs

I would fundamentally redesign the system around multiple independent data domains.

It should not be one large application with hundreds of exchange adapters.

It should be a platform with independently scalable collection systems, governed by one common data architecture.

flowchart TB
    subgraph SOURCES["GLOBAL SOURCE UNIVERSE"]
        A[Stock and Derivatives Exchanges]
        B[OTC and Dealer Networks]
        C[FX and Commodity Markets]
        D[Crypto and Blockchain]
        E[Regulators and Government]
        F[Central Banks and Issuers]
        G[News and Alternative Data]
        H[Commercial Data Vendors]
    end

    subgraph CAPTURE["ACQUISITION INFRASTRUCTURE"]
        I[Exchange Native Feed Connectors]
        J[FIX / SBE / ITCH / FAST]
        K[Multicast / TCP / WebSocket]
        L[REST / FIX APIs / Files]
        M[Documents and Batch Ingestion]
    end

    subgraph CORE["FIELD BOOK DATA CORE"]
        N[Raw Immutable Capture]
        O[Protocol Decoding]
        P[Canonical Event Normalization]
        Q[Instrument and Entity Master]
        R[Reference and Symbology Graph]
        S[Market State Reconstruction]
        T[Data Quality and Reconciliation]
    end

    subgraph STORAGE["DATA PRESERVATION"]
        U[Original Source Archive]
        V[Normalized Event History]
        W[Historical Data Lakehouse]
        X[Hot Market State Store]
        Y[Reference and Document Store]
    end

    subgraph ACCESS["DATA ACCESS"]
        Z[Live Data APIs]
        AA[Historical Query APIs]
        AB[Bulk Downloads]
        AC[Streaming Feeds]
        AD[Search and Discovery]
        AE[SDKs and Integrations]
    end

    A --> I
    B --> L
    C --> J
    D --> K
    E --> M
    F --> M
    G --> M
    H --> L

    I --> N
    J --> N
    K --> N
    L --> N
    M --> N

    N --> U
    N --> O
    O --> P
    Q --> P
    R --> Q
    P --> S
    P --> T
    S --> X
    T --> V
    P --> V
    V --> W
    O --> Y

    U --> Z
    V --> AA
    W --> AB
    X --> AC
    Y --> AD
    Z --> AE
    AA --> AE
This architecture needs six fundamental planes.

### 3.1 Global source acquisition plane

Every source needs an individual connector, but all connectors must implement a common contract.

For example:

trait DataSourceConnector {
    async fn connect(&mut self) -> Result<()>;

    async fn discover(&self) -> Result<Vec<InstrumentReference>>;

    async fn subscribe(
        &mut self,
        instruments: &[InstrumentId],
        subscriptions: &[DataSubscription],
    ) -> Result<()>;

    async fn recover(&mut self) -> Result<RecoveryReport>;

    async fn health(&self) -> Result<SourceHealth>;

    async fn disconnect(&mut self) -> Result<()>;
}
This is illustrative, not a proposed drop-in implementation for the existing code.

More importantly, every connector needs a machine-readable capability declaration.

Example:

`yaml
source: example_exchange
venue_mic: XABC
country: XX

asset_classes:
  - EQUITY
  - ETF
  - EQUITY_OPTION

protocols:
  - NATIVE_BINARY
  - TCP
  - MULTICAST

data_capabilities:
  - TRADES
  - L1_QUOTES
  - L2_BOOK
  - L3_BOOK
  - AUCTION_IMBALANCE
  - MARKET_STATUS
  - REFERENCE_DATA

book_model: ORDER_BY_ORDER

∆, [21.11.2026 05:42]
timestamps:
  exchange_timestamp: true
  sequence_numbers: true
  packet_sequence: true

recovery:
  snapshot: true
  incremental_recovery: true

historical:
  available: true
  depth: EXCHANGE_DEFINED

licensing:
  real_time: CONTRACT_REQUIRED
  historical: CONTRACT_REQUIRED
  redistribution: CONTRACT_REQUIRED

Every actual connector would have a much more detailed manifest, including supported message types, authentication, subscriptions, rate limits, entitlement conditions, market sessions, recovery semantics and schema versions.

The key principle:

A connector must declare what it genuinely supports. It cannot advertise L3 simply because it can create a data structure called an order book.

### 3.2 Raw capture plane

This should be one of Fieldbook's most important competitive advantages.

I would preserve the complete original source information before normalization.

For every captured event, retain:

* Original payload bytes.

* Original protocol and message type.

* Source identity.

* Exchange or contributor identity.

* Network and session metadata.

* Packet sequence, where applicable.

* Exchange timestamp.

* Local receive timestamp.

* Capture location.

* Connection and subscription state.

* Decoder version.

* Raw record checksum.

Preserve original packet boundaries for packet-based feeds, not merely reconstructed messages.

For application feeds, retain original application payloads and appropriate transport metadata.

Store the original information even if the decoder rejects it.

Why?

Five years later, you may write a better decoder, discover an exchange protocol change or need to establish precisely what the exchange sent.

If you discarded the original information during normalization, you cannot recover it.

I would make the raw archive immutable and independently verifiable.

### 3.3 Universal financial data model

This is the most important piece of software to design correctly.

Do not create one giant `MarketDataEvent` with a handful of generic fields.

Different assets have fundamentally different semantics.

An equity has a listing, shares, corporate actions and a trading venue.

A bond has coupon terms, maturity, accrued interest, yield conventions and potentially dealer quotes.

An option has an underlying, strike, expiry, exercise style and deliverable.

An FX quote has a currency-pair convention and value date.

A physical commodity assessment has a location, grade, delivery window and unit of measurement.

A weather observation has a measurement station, observation time and physical units.

These are not interchangeable.

I would build a universal event envelope with strongly typed domain payloads.

For example:

rust
struct DataEventEnvelope {
    event_id: EventId,

    source_id: SourceId,
    instrument_id: Option<InstrumentId>,
    entity_id: Option<EntityId>,

    event_type: DataEventType,

    event_time: Timestamp,
    receive_time: Timestamp,
    ingest_time: Timestamp,

    venue: Option<VenueId>,
    country: Option<CountryCode>,

    sequence: Option<u64>,

    source_schema_version: SchemaVersion,
    normalizer_version: Version,

    quality: DataQuality,
    provenance: Provenance,

    raw_record_id: RawRecordId,
}

The envelope would contain a domain-specific payload, not an untyped JSON dictionary.

The domain models would include:

text
EquityQuote
EquityTrade
OrderAdded
OrderModified
OrderCancelled

BondQuote
BondTrade
EvaluatedBondPrice

FXQuote
FXTrade
FXForwardPoint

FuturesContract
FuturesQuote
FuturesTrade

OptionContract
OptionQuote
OptionTrade
OptionVolatilitySurface

CommodityAssessment
CommodityInventory
CommodityCurve

EconomicObservation
EconomicRelease
CorporateAction
CompanyFundamental
RegulatoryFiling
NewsEvent
BlockchainEvent
`

There should also be extension mechanisms for source-specific information that does not fit neatly into a common model.

Never silently discard a source-specific field because the universal schema does not yet understand it.

### 3.4 The universal instrument and entity master

∆, [21.11.2026 05:42]
This deserves its own major subsystem.

In fact, I would consider this one of the hardest and most valuable parts of the entire platform.

A ticker is not a universal instrument identifier.

The same company can have several share classes, listings, currencies and exchange identifiers. A derivative can have thousands of contract combinations. A future's identity changes across expiry cycles.

Build an internal permanent identifier system and an identity graph.

External reference standards should be integrated rather than reinvented.

| Standard         | Purpose                                         |
| ---------------- | ----------------------------------------------- |
| ISIN (ISO 6166)  | Securities identification                       |
| FIGI             | Financial instrument identification and mapping |
| MIC (ISO 10383)  | Market and venue identification                 |
| CFI (ISO 10962)  | Financial instrument classification             |
| FISN (ISO 18774) | Financial instrument short name                 |
| LEI (ISO 17442)  | Legal entity identification                     |
| UPI (ISO 4914)   | OTC derivatives product identification          |
| UTI (ISO 23897)  | Unique transaction identification               |
| DTI (ISO 24165)  | Digital token identification                    |
| ISO 4217         | Currency identification                         |

These standards address different identity problems; none replaces all the others. October 2026 +5

Use GLEIF as an important entity-reference source. Its Global LEI Index contains current and historical legal-entity records. LEI Data – GLEIF

Use OpenFIGI for identifier mapping and enrichment, but not as your only instrument master. Its documentation notes that unlisted equities are excluded by default from mapping results. OpenFIGI +1

Your internal instrument model must distinguish:

Entity
  └── Issuer
        └── Financial Instrument
              └── Listing
                    └── Venue
                          └── Trading Session

Financial Instrument
  └── Underlying
  └── Contract Terms
  └── Currency
  └── Expiry
  └── Corporate Action History
  └── External Identifier Mappings
Also support effective-dated identifiers: what an instrument was called before a corporate action must remain discoverable.

The same applies to company mergers, reorganizations, delistings and renamed securities.

### 3.5 Data quality and reconciliation

I would make data-quality assurance a first-class subsystem, not a collection of metrics.

For every source, continuously evaluate:

* Sequence gaps.

* Missing messages.

* Duplicate messages.

* Out-of-order messages.

* Unexpected message types.

* Incorrect numerical scales.

* Invalid instrument mappings.

* Stale quotes.

* Crossed or locked books.

* Invalid book quantities.

* Trading-session inconsistencies.

* Timestamp anomalies.

* Missing trades.

* Failed snapshot reconstruction.

* Incorrect corporate-action adjustments.

* Conflicting reference data.

Maintain independent quality states:

RAW_CAPTURED → DECODED → NORMALIZED → VALIDATED → RECONCILED

A failed event must never simply disappear.

It should carry a quality status and a machine-readable reason.

For a source where exchange sequence numbers are available, establish continuity and reconcile gaps.

For a source where they are unavailable, clearly declare that limitation and use the strongest validation the source permits.

The target should be zero silent data loss, not an unrealistic guarantee that an external exchange can never lose a message.

### 3.6 Historical storage and preservation

I recommend three distinct storage layers.

| Layer      | Purpose                                                 | Suggested technology                                       |

∆, [21.11.2026 05:42]
| ---------- | ------------------------------------------------------- | ---------------------------------------------------------- |
| Raw        | Original captured records and packet data               | Immutable object storage                                   |
| Normalized | Historical event records                                | Apache Parquet + Apache Iceberg                            |
| Hot        | Frequently queried market state and recent observations | In-memory state and a high-performance analytical database |

For historical datasets, Apache Iceberg is worth serious consideration.

Its schema evolution, partition evolution and time-travel capabilities are useful for continuously expanding financial datasets. Apache Iceberg™ +1

I would also support:

* Full historical replay.

* Versioned datasets.

* Historical corrections.

* Snapshot reconstruction.

* Source-by-source comparisons.

* Reprocessing with new decoders.

* Reproducible queries against a particular historical dataset version.

Historical data should be treated as a permanent product, not a temporary cache.

### 4\. Data collection protocols: support the actual financial world

Fieldbook should not be restricted to REST and WebSocket integrations.

It needs a protocol infrastructure capable of handling different classes of sources.

| Protocol / transport | Application                                          |
| -------------------- | ---------------------------------------------------- |
| FIX                  | Financial information and market-data messaging      |
| FIX Orchestra        | Machine-readable messaging and interface definitions |
| SBE                  | Efficient binary encoding                            |
| ITCH                 | Order-level exchange messages                        |
| FAST                 | Compact financial messaging                          |
| UDP multicast        | Exchange market-data distribution                    |
| TCP                  | Reliable source connections                          |
| WebSocket            | Streaming APIs                                       |
| REST                 | Reference and historical APIs                        |
| SFTP / files         | Historical and batch data                            |
| Proprietary binary   | Venue-specific feed protocols                        |
| Blockchain RPC       | Digital asset and on-chain data                      |
| Document ingestion   | Filings, reports and announcements                   |

The FIX Trading Community maintains FIX specifications and the Orchestra standard for machine-readable protocol definitions. These should form part of Fieldbook's protocol framework. FIX Trading Community +1

A crucial distinction: FIX is not a substitute for native exchange feeds. Fieldbook needs both standardized integration and exchange-specific implementations.

### Timestamp architecture

This needs serious attention from the beginning.

Every event should preserve separate timestamps:

* Exchange event time.

* Exchange publication time, if available.

* Local network receive time.

* Application receive time.

* Normalization time.

* Durable-write time.

Also store:

* UTC offset.

* Clock synchronization source.

* Clock uncertainty.

* Timestamp precision.

* Monotonic clock measurement.

* Source-specific time semantics.

Use UTC consistently, with properly synchronized system clocks and PTP where appropriate.

This is not merely a performance concern. Timestamp synchronization is explicitly addressed in European market infrastructure regulation, including MiFIR Article 22c and its associated technical rules. European Securities and Markets Authority +1

I would never use one generic timestamp field and expect it to serve all these purposes.

### 5\. The biggest business and engineering challenge: obtaining the information

There is something I want to emphasize because it could determine whether this project succeeds or fails.

The world's financial information is not available through one universal API.

∆, [21.11.2026 05:42]
There are thousands of different data sources, each with different connectivity, commercial terms, coverage, formats and distribution rights.

A technically excellent collector cannot collect data to which it has no authorized access.

Consider the licensing situation:

* The London Stock Exchange requires data distributors to sign a licence agreement to receive and redistribute its data. marketdata.londonstockexchange.com

* Euronext distinguishes internal use, display use, non-display use and redistribution. Redistribution of real-time data requires appropriate licensing and fees. Euronext Data

This means Fieldbook needs an entirely separate Data Rights and Entitlements Management System.

### 5.1 Build a global licensing registry

For every source, record:

| Field               | Example                              |
| ------------------- | ------------------------------------ |
| Provider            | Exchange or data vendor              |
| Territory           | Country or permitted jurisdiction    |
| Product             | Level 1, Level 2, trades, historical |
| Real-time rights    | Permitted or prohibited              |
| Historical storage  | Permitted or prohibited              |
| Internal processing | Permitted or prohibited              |
| Derived data        | Permitted or prohibited              |
| Redistribution      | Permitted or prohibited              |
| Display rights      | Permitted or prohibited              |
| Non-display rights  | Permitted or prohibited              |
| User restrictions   | Contract-specific                    |
| Retention           | Contract-specific                    |
| Audit requirements  | Contract-specific                    |
| Expiry              | Contract-specific                    |

The permissions system must be capable of restricting access to individual datasets.

Do not assume that because you can access information through a broker account or a free API, you can store it indefinitely or redistribute it through your own service.

The objective is not simply to collect everything. It is to collect everything that can lawfully be collected, preserve it, and make it available under the rights applicable to each dataset.

### 5.2 Build a global source acquisition programme

I would maintain four categories of acquisition.

Category 1: Direct exchange and market-infrastructure integrations

Prioritize direct exchange relationships, official feeds, official historical-data services and authorized market infrastructure.

This gives you the best chance of obtaining the original market event stream.

Category 2: Licensed data vendors

This is essential for countries, instruments and asset classes where individual exchange integrations would take too long or be commercially impractical.

Think of major vendors and specialist data providers.

Category 3: Official government and institutional sources

Examples include:

* Central banks.

* National statistics agencies.

* Securities regulators.

* Treasury departments.

* Government debt agencies.

* Commodity agencies.

* Company registries.

* National exchange regulators.

* International financial institutions.

Category 4: Specialized information providers

For example:

* Credit-rating providers.

* Commodity price-assessment providers.

* Fixed-income data providers.

* Corporate financial information providers.

* News providers.

* Alternative-data providers.

Each source gets a contractual and technical onboarding process.

### 5.3 How I would approach the 90-country objective

Do not build 90 country-specific collectors.

Build a global collection framework that can accommodate country-specific differences.

For each country, create a market coverage dossier.

For instance:

| Country        | Equity   | Fixed income | Derivatives | FX       | Macro    | Corporate |

∆, [21.11.2026 05:42]
| -------------- | -------- | ------------ | ----------- | -------- | -------- | --------- |
| United States  | Required | Required     | Required    | Required | Required | Required  |
| United Kingdom | Required | Required     | Required    | Required | Required | Required  |
| India          | Required | Required     | Required    | Required | Required | Required  |
| Japan          | Required | Required     | Required    | Required | Required | Required  |
| Germany        | Required | Required     | Required    | Required | Required | Required  |
| Brazil         | Required | Required     | Required    | Required | Required | Required  |
| South Africa   | Required | Required     | Required    | Required | Required | Required  |
| Singapore      | Required | Required     | Required    | Required | Required | Required  |
| South Korea    | Required | Required     | Required    | Required | Required | Required  |
| Saudi Arabia   | Required | Required     | Required    | Required | Required | Required  |

This is an intended coverage matrix, not a claim that every category is exchange-traded or publicly available in every country.

Every country needs:

* Country metadata.

* Official exchange registry.

* Venue registry.

* Market calendars.

* Public holidays.

* Trading sessions.

* Currency and settlement conventions.

* Regulators and official sources.

* Corporate reporting sources.

* Economic-data sources.

* Local identifiers.

* Local language and document handling.

* Licensing and data-residency restrictions.

And there needs to be a formal definition of country coverage.

For example, a market is not fully covered simply because you can retrieve yesterday's closing price.

It might have:

* Daily EOD prices.

* Real-time L1.

* Real-time L2.

* L3, where the venue offers it.

* Historical trades.

* Corporate actions.

* Reference information.

* Financial statements.

All are different coverage achievements.

I would expose this distinction to users instead of hiding it.

### 6\. What makes Fieldbook genuinely better than existing providers?

This is where I would be particularly ambitious.

Do not try to beat Bloomberg by creating another Bloomberg terminal.

Do not try to beat LSEG by creating another market-data display application.

Build the best underlying data acquisition, verification, history and accessibility infrastructure.

I would establish the following ten principles.

### 1\. No silent data loss

Every received message must be accounted for.

Every sequence gap must be identifiable.

Every failed decoder must be recorded.

Every recovery attempt must be auditable.

Where the source itself does not provide sufficient information to guarantee completeness, report that limitation.

### 2\. No destructive normalization

Never discard the source's original information.

Maintain:

* Original raw data.

* Original source identifiers.

* Original numerical representations.

* Normalized representations.

* Derived calculations.

* Links between all of them.

### 3\. No universal fake order book

Never combine fundamentally different liquidity sources into a single supposedly authoritative book.

Preserve the original venue and contributor.

Provide separate consolidated views only when the methodology, coverage and permissions justify them.

### 4\. No invented completeness

Every instrument and source must disclose the actual availability of each information type.

A missing field is different from a zero value.

An unavailable price is different from no trading activity.

A stale price is different from a current price.

### 5\. Historical information must be permanent

Preserve:

* Original releases.

* Subsequent revisions.

* Corrections.

* Delisted instruments.

* Expired derivatives.

* Historical symbols.

* Old company identities.

* Old contract specifications.

* Previous market-session states.

Never make historical queries depend exclusively on current reference data.

### 6\. Every data point must have provenance

I would make it possible to answer:

* Who published this?

∆, [21.11.2026 05:42]
* Which venue or organization produced it?

* When did it happen?

* When did Fieldbook receive it?

* Was it directly observed or calculated?

* Was it subsequently corrected?

* Which decoder generated the normalized result?

* Which source licence governs its use?

### 7\. No asset-class-specific architectural limitations

An equity connector must not be necessary to represent an instrument.

An FX connector must not be necessary to represent every currency pair.

A commodity assessment must not need to masquerade as a security.

The domain model must accommodate new instruments without architectural restructuring.

### 8\. Research-grade historical fidelity

An analyst must be able to reproduce the same historical dataset, including the relevant corrections, metadata version and transformations.

### 9\. Transparent source comparison

Where multiple independent sources cover the same instrument, make their differences available.

For example, compare:

* Venue A versus Venue B.

* Exchange price versus vendor price.

* Original close versus adjusted close.

* Indicative versus executable quotes.

* Official economic release versus subsequent revision.

### 10\. Independent verification

Build automated testing and reconciliation against exchange reference outputs, official publications and trusted independent sources.

Never allow the adapter developer to be the sole judge of whether an adapter works correctly.

### 7\. Concrete technology architecture

I would retain Rust as a major part of Fieldbook, but I would not insist that absolutely every component must be written in Rust.

The correct technology should be selected according to the workload.

| Layer                       | Recommended technology direction                                                                 |
| --------------------------- | ------------------------------------------------------------------------------------------------ |
| High-performance collectors | Rust, with C/C++ interoperability where useful                                                   |
| Protocol parsers            | Rust and generated protocol decoders                                                             |
| Exchange feed state         | Rust                                                                                             |
| Streaming transport         | Kafka/Redpanda for durable distributed ingestion; specialized low-latency transport where needed |
| Raw data                    | Immutable object storage                                                                         |
| Historical storage          | Parquet + Iceberg                                                                                |
| Analytical queries          | ClickHouse, DuckDB, Trino, and KDB-X where appropriate                                           |
| Metadata databases          | PostgreSQL                                                                                       |
| Search                      | OpenSearch or equivalent                                                                         |
| Time-series state           | Purpose-built in-memory structures and analytical stores                                         |
| Schema management           | Versioned schemas and a schema registry                                                          |
| API                         | gRPC, REST, WebSocket and bulk file access                                                       |
| Metrics                     | Prometheus and Grafana                                                                           |
| Distributed tracing         | OpenTelemetry                                                                                    |
| Deployment                  | Containerized services, infrastructure as code                                                   |
| Security                    | Encryption, secrets management, identity and entitlement services                                |

∆, [21.11.2026 05:42]
These are proposed technology choices, not requirements to add every product immediately.

### Separate two processing paths

A particularly important design decision:

Real-time path

Optimized for receiving and making current data available with predictable latency.

Source
  → Capture
  → Decode
  → Normalize
  → Validate
  → Live data distribution
Historical preservation path

Optimized for durability, completeness, reprocessing and auditability.

Source
  → Raw journal
  → Immutable archive
  → Normalized event history
  → Reconciliation
  → Historical lakehouse
Both paths must be independently recoverable.

Neither should depend on the other being healthy.

A slow historical archive must not automatically block the real-time ingestion path. Conversely, a restart of the live processing system must not corrupt the historical archive.

And do not spend all your resources optimizing microsecond latency for slow-moving fundamental or macroeconomic data.

Use different service-level targets for different data classes.

An exchange multicast feed requires very different engineering from a daily central-bank statistical release.

### 8\. What needs to change in the existing Fieldbook repository?

I would not throw away the work that has already been done.

I would preserve its strongest engineering decisions, but change the architecture's centre of gravity.

The current implementation is principally an exchange market-data collector. The future implementation must become a universal financial information collection platform.

Here is my assessment of the existing components.

| Existing Fieldbook component | Decision                        | Required evolution                                                 |
| ---------------------------- | ------------------------------- | ------------------------------------------------------------------ |
| Rust foundation              | Keep                            | Retain as a major acquisition technology                           |
| Tokio connectivity           | Keep                            | Add dedicated native-feed networking                               |
| CollectorCore                | Evolve                          | Become a source-independent ingestion and event-processing core    |
| Price and Quantity types     | Redesign                        | Support typed financial prices, yields, rates, units and precision |
| OrderBook                    | Expand                          | L1, L2 and L3 plus non-book market data                            |
| Exchange adapters            | Rebuild framework               | Universal source connector interface                               |
| Coordinator                  | Keep and expand                 | Global regional collection orchestration                           |
| Shadow books and recovery    | Keep                            | Generalize recovery capabilities by source type                    |
| SBE writer                   | Keep as an option               | Support multiple data serialization formats                        |
| Raw-frame journal            | Preserve and expand             | Native packet and application-payload preservation                 |
| Prometheus metrics           | Keep                            | Add coverage, data quality and entitlement monitoring              |
| Replay tooling               | Expand                          | Historical event replay and decoder migration                      |
| Parquet export               | Evolve                          | Permanent historical lakehouse                                     |
| KDB-X analytics              | Keep as an optional integration | Support broader analytical consumers                               |

The existing OrderBook should no longer define the whole identity of Fieldbook.

It should become one specialized state engine among many.

For example:

∆, [21.11.2026 05:42]
Fieldbook
│
├── Acquisition
│   ├── Exchange Connectors
│   ├── Vendor Connectors
│   ├── Government Connectors
│   ├── Issuer Connectors
│   ├── News Connectors
│   └── Alternative Data Connectors
│
├── Financial Domain
│   ├── Instrument Master
│   ├── Entity Master
│   ├── Equity Domain
│   ├── Fixed Income Domain
│   ├── Derivatives Domain
│   ├── FX Domain
│   ├── Commodity Domain
│   ├── Digital Asset Domain
│   ├── Macro Domain
│   └── Corporate Information Domain
│
├── Market State
│   ├── Quote State
│   ├── L1 Engine
│   ├── L2 Engine
│   ├── L3 Engine
│   ├── Trade State
│   ├── Auction State
│   └── Market Session State
│
├── Data Integrity
│   ├── Raw Journal
│   ├── Sequence Validation
│   ├── Reconciliation
│   ├── Timestamp Validation
│   ├── Provenance
│   └── Replay
│
├── Storage
│   ├── Raw Archive
│   ├── Event Store
│   ├── Historical Lakehouse
│   ├── Reference Store
│   └── Document Archive
│
├── Governance
│   ├── Source Registry
│   ├── Licensing
│   ├── Entitlements
│   ├── Data Retention
│   └── Audit
│
└── Distribution
    ├── Streaming APIs
    ├── Historical APIs
    ├── Bulk Data
    ├── Search
    └── SDKs
I would also establish separate domain modules and fault boundaries rather than turning this into an enormous microservice deployment immediately.

Develop a modular codebase first. Split processes and services according to scaling, security and operational requirements.

### 9\. The development roadmap I would follow

I would not start by adding 100 more exchange connectors.

That would compound the existing architectural limitations.

I would follow these milestones.

### Milestone 1: Universal data foundation

Before adding large numbers of sources:

* Finalize the universal event envelope.

* Finalize the instrument and entity identity graph.

* Create the global venue and source registry.

* Create the financial instrument taxonomy.

* Create the data rights system.

* Define the timestamp model.

* Define schema compatibility and versioning rules.

* Define the connector contract.

* Define the independent data-quality framework.

Acceptance: A new asset class can be represented without redesigning the entire core.

### Milestone 2: Institutional-grade acquisition engine

Upgrade the existing Fieldbook capture system.

Implement:

* TCP and UDP acquisition.

* Multicast support.

* FIX sessions.

* Native exchange binary protocols.

* WebSocket and REST.

* File and batch acquisition.

* Connection supervision.

* Reconnection and recovery.

* Redundancy.

* High-throughput raw journals.

* Independent capture and persistence paths.

Acceptance: Multiple independent sources can feed the same normalized data platform without requiring source-specific storage or analytics systems.

### Milestone 3: Complete market-data capability

Implement the event and state engines.

| Function  | Required result                           |
| --------- | ----------------------------------------- |
| L1        | Best bid/ask                              |
| L2        | Aggregated depth                          |
| L3        | Individual order events where offered     |
| Trades    | Individual transaction events             |
| Auctions  | Auction prices and imbalance information  |
| Quotes    | Venue and contributor-specific quotations |
| Status    | Market sessions, halts and interruptions  |
| Reference | Instrument and contract specifications    |
| Curves    | Futures, rates, FX and commodity curves   |
| Options   | Chains, volatility and contract metadata  |

Acceptance: The system can correctly process different market-data models rather than assuming everything is an order book.

### Milestone 4: Global market coverage

Build official source integrations and onboard authorized providers.

I would begin with geographically and structurally diverse markets rather than concentrating all development on one region.

Build coverage for:

* North America.

* Europe.

* South Asia.

* East Asia.

* Southeast Asia.

* Latin America.

* Middle East.

* Africa.

* Oceania.

∆, [21.11.2026 05:42]
Expand country by country, using actual exchange and instrument coverage measurements.

Acceptance: Every production source has documented coverage, licensing, quality, historical availability and operating status.

### Milestone 5: Institutional financial information

This is where Fieldbook becomes more than a market-data collector.

Integrate:

* Company filings.

* Financial statements.

* Corporate actions.

* Macroeconomic releases.

* Central-bank data.

* Government debt information.

* Ratings and credit data.

* News.

* Commodity assessments.

* Fund data.

* Research and estimates.

* Alternative datasets.

Acceptance: Users can search an instrument or entity and discover its market data, reference information, financial history and associated publications, with provenance.

### Milestone 6: Historical data platform

Build the complete historical system:

* Immutable source archive.

* Historical event database.

* Historical order books.

* Historical trades.

* Historical instrument metadata.

* Revision history.

* Corrected data.

* Historical corporate actions.

* Historical document archive.

* Reproducible datasets.

Acceptance: Every historical result can be tied to its sources and data-version history.

### Milestone 7: Independent reliability certification

The entire platform must be tested under realistic conditions.

Include:

* 24/7 soak tests.

* Failure injection.

* Network disconnection.

* Packet loss.

* Duplicate delivery.

* Reordered messages.

* High-volume bursts.

* Storage failures.

* Disk exhaustion.

* Clock deviations.

* Process restarts.

* Regional network failures.

* Schema changes.

* Historical rebuilds.

* Recovery after prolonged outages.

A release should not be considered production-certified simply because the Rust test suite passes.

### 10\. Define measurable standards that cannot be compromised

I would introduce a Fieldbook Data Quality Standard.

Every connector and dataset must pass the same fundamental quality framework.

| Dimension           | Required property                                |
| ------------------- | ------------------------------------------------ |
| Provenance          | Every observation traces to its source           |
| Completeness        | No silent gaps                                   |
| Sequence integrity  | Verified where sequence information exists       |
| Precision           | No unintended numerical loss                     |
| Time integrity      | Multiple timestamps and synchronization evidence |
| Recovery            | Demonstrated recovery behavior                   |
| Historical fidelity | Original data remains available                  |
| Schema integrity    | Versioned and validated                          |
| Reference integrity | Instrument mappings are auditable                |
| Corrections         | Append-only revision history                     |
| Licensing           | Usage rights machine-enforceable                 |
| Reproducibility     | Historical results can be reconstructed          |

I would additionally measure:

Source coverage: Percentage of eligible instruments and event types actually supported.

Data completeness: Expected versus received source events, where the source provides a measurable denominator.

Freshness: Source publication time versus receipt time.

Recovery time: How quickly missing or interrupted data is reconciled.

Data correctness: Independent reconstruction and validation success.

Historical depth: How much verified historical data is actually retained.

Availability: Live collection availability by source, session and data product.

Importantly, these metrics should be published per source and dataset, not merely as one platform-wide availability percentage.

A platform could be healthy overall while one important exchange connector is silently broken.

That must never be hidden by aggregate metrics.

### 11\. What Fieldbook must never become

I want to draw some boundaries because they will help maintain your vision.

∆, [21.11.2026 05:42]
You are building a data company and data infrastructure platform, not a trading company.

I would explicitly exclude:

* Order placement.

* Broker connectivity for execution.

* Portfolio management.

* Strategy execution.

* Trading algorithms.

* Trade routing.

* Position management.

* Pre-trade risk engines.

However, collecting actual exchange order events, trade prints, auction activity, transaction reports and historical order-book information is absolutely within scope.

The distinction is between observing market activity and participating in it.

You can build excellent execution-market research data without building execution software.

### 12\. The reality of competing with the best

There is an important limitation to the ambition of collecting everything.

Some financial information is private, bilateral, unpublished, restricted, non-reported or commercially inaccessible. Some order books are only partially displayed. Certain OTC markets do not expose complete transaction-level information. Some alternative information requires separate permission.

No architecture can overcome information that its source does not disclose.

Therefore, the goal must be:

The most comprehensive, accurate, traceable and accessible collection of all information available through authorized sources—not a false promise of access to information that does not exist publicly or cannot legally be obtained.

I would measure Fieldbook against five established categories of competitors:

| Benchmark                | What Fieldbook should aspire to match or exceed |
| ------------------------ | ----------------------------------------------- |
| Bloomberg                | Breadth of financial information                |
| LSEG                     | Global data coverage and reference information  |
| ICE Data Services        | Cross-asset information and pricing             |
| Databento                | Raw market-data fidelity and technical capture  |
| Nasdaq direct data feeds | Exchange-level market-depth fidelity            |

The scale is substantial. LSEG's publicly described coverage of 100 million-plus instruments and 500-plus venues and contributors illustrates how much infrastructure a genuinely global data business requires. Data Analytics

I would not claim Fieldbook is competitive with these providers today. The existing repository is an early technical foundation.

But I would absolutely use these companies as the standards against which the future architecture is evaluated.

### 13\. My most important recommendation: introduce a Fieldbook Coverage Certification System

I would make this a defining product feature.

Every source, country, market and instrument should have a visible, independently measurable status.

For example:

| Status                 | Meaning                                    |
| ---------------------- | ------------------------------------------ |
| Discovered             | Source identified                          |
| Connector implemented  | Integration exists                         |
| Technically validated  | Protocol and parsing tested                |
| Licensed               | Appropriate permissions verified           |
| Live                   | Production ingestion active                |
| Historically populated | Historical records available               |
| Reconciled             | Independent data checks completed          |
| Certified              | All applicable quality requirements passed |

For an individual instrument, users should be able to inspect:

* Available data categories.

* Available market depth.

* Historical date range.

* Real-time or delayed status.

* Trading venues.

* Source providers.

* Last successful collection.

* Known gaps.

* Data-quality status.

* Correction history.

* Applicable usage restrictions.

This turns transparency into a competitive advantage.

∆, [21.11.2026 05:42]
A user should never have to wonder whether a missing price means no trading occurred, an exchange feed failed, the instrument is unsupported, or the source simply does not provide that information.

### 14\. The final vision

I would describe the finished product as follows:

# FIELDBOOK
## Global Financial and Economic Data Infrastructure

Mission

To build the world's most comprehensive financial and economic information collection infrastructure.

Fieldbook acquires, preserves, verifies, normalizes, indexes and distributes financial and economic information from authorized sources across global markets.

Coverage

- 90+ countries and their financial markets.
- All major financial asset classes.
- Exchange-traded and OTC instruments where data is available.
- Full available market depth, quotes and transactions.
- Historical and real-time financial information.
- Financial instrument and entity reference data.
- Corporate and financial information.
- Macroeconomic and government information.
- Commodities and physical markets.
- Digital assets and blockchain information.
- News, regulatory announcements and corporate events.
- Licensed alternative information.

Engineering principles

1. Original information is preserved.
2. No silent data loss.
3. No destructive normalization.
4. Every data point has traceable provenance.
5. Historical information and revisions are retained.
6. Coverage and limitations are transparent.
7. Data rights are enforced.
8. Data quality is independently verifiable.
9. All supported asset classes have appropriate domain models.
10. New sources and instruments can be integrated without architectural redesign.

Product boundary

Fieldbook is exclusively a data acquisition, preservation, intelligence, historical research and distribution platform.

It does not execute trades, route orders, operate brokerage accounts or manage investment portfolios.

Ultimate objective

Provide users with the most comprehensive and trustworthy view of global financial and economic information possible through authorized data sources.

### 15\. What I would do immediately

My recommendation is to stop treating the existing repository as something that simply needs additional exchange integrations.

Treat it as Fieldbook V1, and establish a formal Fieldbook V2: Global Data Infrastructure Architecture.

Before writing hundreds of additional connectors, I would make six technical specifications mandatory:

| Specification                   | Deliverable                                                        |
| ------------------------------- | ------------------------------------------------------------------ |
| 01. Universal Data Taxonomy     | Complete asset, instrument and information classification          |
| 02. Universal Instrument Master | Entity graph, identifiers, listings, contracts and history         |
| 03. Universal Event Model       | Typed schemas for all supported data domains                       |
| 04. Acquisition Framework       | Connector SDK, source registry and protocol architecture           |
| 05. Data Integrity Standard     | Capture, timestamps, recovery, reconciliation and certification    |
| 06. Global Coverage and Rights  | 90-country market map, source inventory and licensing architecture |

Then refactor Fieldbook against those specifications.

One final thought: I would prioritize being the world's most trustworthy collector before trying to be the largest collector.

A system that collects 10 million instruments with verifiable provenance, correct historical reconstruction and transparent coverage is more valuable than one claiming 100 million instruments while quietly losing, misidentifying or corrupting data.

The long-term ambition should be both: exceptional data integrity and extraordinary global breadth. Neither should be sacrificed for the other.
