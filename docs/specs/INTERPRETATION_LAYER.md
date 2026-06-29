---
author: Ray
title: 解釈層 (Interpretation Layer)
description: Trend, Gravity, Supply, Expectation を自然言語へ翻訳し、Future Hypothesis Layer の前段に置く表示専用層の仕様。
key: interpretation-layer
---

# 解釈層 (Interpretation Layer)

Interpretation Layer は、複数の Observation Layer の結論を人間が読める自然言語へ翻訳する表示専用層である。

この層は新しい事実を生成しない。既存の観測結果をまとめ、なぜその見え方になっているのかを説明する。

## 1. 目的

本層の目的は、各 Layer が個別に返した観測結果を、ひとつの Narrative にせず、しかし断片のままにもせず、説明可能な文章へ統合することである。

Interpretation Layer が答える問いは次のとおりである。

- 価格はなぜこう動いたのか。
- どの Layer が変化したのか。
- どの Layer は変化していないのか。
- いまの事実は何か。
- 何が市場の期待で、何が資金行動で、何が長期仮説なのか。

Interpretation Layer は、次の問いには答えない。

- 何を売買すべきか。
- どの Gate を通すべきか。
- どの Position Sizing を選ぶべきか。

## 2. 配置

報告上の配置は次の順序とする。

```text
Trend Layer
Gravity Layer
Supply Layer
Expectation Layer
Interpretation Layer
Future Hypothesis Layer
```

Interpretation Layer は Trend Layer と Future Hypothesis Layer の間に置く。

Future Hypothesis Layer は将来の仮説を表示する層であり、Interpretation Layer がその前段で現在の観測を説明する。

## 3. 責務分離

各 Layer の責務は次のとおりである。

| Layer | 責務 |
|---|---|
| Trend | 価格がどう動いたかを答える。 |
| Gravity | 価格が価値に対してどこにあるかを答える。 |
| Expectation | 市場がすでに何を信じているかを答える。 |
| Supply | 将来の供給圧力があるかを答える。 |
| Flow | 資金が現在の趨勢を支えているかを答える。 |
| Interpretation | 上記の観測を自然言語へ翻訳する。 |
| Future Hypothesis | まだ事実ではない将来仮説を示す。 |

Interpretation Layer は他の Layer を上書きしない。

Interpretation Layer は、他の Layer が示した結論を言い換えるだけであり、独自の trading signal を生成しない。

```text
Current decision weight = 0%
```

したがって、Interpretation Layer は次に影響しない。

- Gate
- Execution
- Trader
- Action Matrix
- READY
- Position Sizing

## 4. Expectation Quality

Expectation Layer では、`Expected: unavailable` のような単一表記だけでは意味が曖昧になる。

そのため、Expectation Layer には `Expectation Quality` を追加する。

### 4.1 値

- `HIGH`
- `MEDIUM`
- `LOW`
- `UNAVAILABLE`

### 4.2 意味

- `HIGH` は、市場一致予想が明確で、システムもその予想を取得できている状態である。
- `MEDIUM` は、予想はあるが粒度が粗い、あるいは一部の補助情報が不足している状態である。
- `LOW` は、予想が弱い、断片的、または信頼性が低い状態である。
- `UNAVAILABLE` は、システムが予想データを取得できていない状態である。

重要なのは、次の 2 つを分けることである。

1. 市場一致予想がそもそもない。
2. システムがまだ予想データを取得できていない。

この 2 つは同じではない。

## 5. Gravity Data Quality

Gravity Layer でも、`Provider unavailable` のような表記だけでは不十分である。

そのため、Gravity Layer には `Data Quality` を追加する。

### 5.1 値

- `READY`
- `PARTIAL`
- `UNAVAILABLE`

### 5.2 意味

- `READY` は、今日の分析に足る水準でデータが揃っている状態である。
- `PARTIAL` は、一部の情報はあるが、完全な分析には足りない状態である。
- `UNAVAILABLE` は、今日のデータが分析に使えない状態である。

### 5.3 Reason

Data Quality は理由を併記する。

理由は少なくとも次を区別する。

- `Provider unavailable`
- `Historical snapshot missing`
- `Consensus unavailable`
- `Source temporarily unavailable`

Data Quality は Gravity の状態そのものではない。

たとえば、Gravity の評価が `Fair` でも、Data Quality は `PARTIAL` になり得る。

## 6. 生成ルール

Interpretation Layer は、各 Layer の状態を見て、次のような説明を生成する。

### 6.1 Event waiting

Trend が弱く、Expectation が未着地で、Supply に新しい圧力がなく、Gravity も極端でない場合、Interpretation は次のように述べる。

- 価格の弱さは、長期 Thesis の崩れではなく、イベント待ちの可能性が高い。

### 6.2 Fundamental pricing

Trend が安定し、Gravity が妥当で、Expectation に大きな変化がない場合、Interpretation は次のように述べる。

- 市場はすでに基本面を取引し始めており、価格は未来期待よりも現実の実現を反映している。

### 6.3 Post-rally consolidation

上昇後に Trend が戻り、Expectation が変わらず、他の観測も悪化していない場合、Interpretation は次のように述べる。

- 現在の下落は、上昇後の通常の整理であり、長期構造の悪化証拠はまだ見えていない。

### 6.4 Supply pressure

Supply に新規の圧力がある場合、Interpretation はその供給が価格の説明にどう効くかを言語化する。

ただし、供給の存在を自動的に売買結論へ変換してはならない。

### 6.5 Flow phase

Flow Layer は Phase 2 で接続する。

Flow が来た後は、Interpretation が資金流入出を説明材料として引用できる。

Phase 1 では Flow がなくても Interpretation を生成できるようにする。

## 7. 文体ルール

Interpretation の文章は、次の性質を持つ。

- 短い
- 断定しすぎない
- 事実と推測を混ぜない
- 交易命令を含めない
- 1 つ以上の観測 Layer を根拠として明示する

推測を使う場合は、`可能性が高い`、`現時点では`、`まだ証拠がない` といった表現に留める。

## 8. 既存 Expectation 行との違い

Expectation Layer の各 observation にある `Interpretation` 行は、個別観測に対する補助説明である。

本層の Interpretation Layer は、それらを上位で束ねた section である。

両者を同じ概念として扱ってはならない。

## 9. 例

### 9.1 TSLA

```text
Trend
連続して弱い。

Expectation
Delivery 待ちで、市場は次の材料を待っている。

Supply
新規の供給圧力は見えていない。

Interpretation
現在の弱さは、長期 Thesis の変化というより、イベント待ちの色が濃い。
```

### 9.2 GOOG

```text
Trend
構造は安定している。

Gravity
評価はおおむね妥当である。

Expectation
大きなイベントはない。

Interpretation
市場はすでに基本面を織り込み始めており、価格は未来期待よりも現実の実現を映している。
```

### 9.3 NVDA

```text
Trend
上昇後の戻り局面にある。

Expectation
大きな変化はない。

Interpretation
現在の下落は、上昇後の通常の整理とみるのが自然であり、長期構造の悪化証拠はまだない。
```

## 10. Phase 計画

### Phase 1

Trend, Gravity, Supply, Expectation, Interpretation を接続する。

この段階では、Flow は未接続でもよい。

### Phase 2

Flow Layer を Observation Layer として接続する。

Interpretation は資金流入出を説明材料として自然に引用できるようにする。

### Phase 3

複数の Observation Layer を自動統合して、より完全な Narrative を生成する。

ただし、Narrative が完成しても、依然として売買判断には接続しない。

## 11. 完了条件

この設計が完了したとみなす条件は次のとおりである。

1. Interpretation Layer が表示専用である。
2. Interpretation Layer が自然言語の説明を生成する。
3. Expectation Quality と Gravity Data Quality が区別されている。
4. Data がないことと、Data をまだ取得できていないことが区別されている。
5. Current decision weight = 0% が明記されている。
6. docs/README.md の specs index からこの文書へ辿れる。

