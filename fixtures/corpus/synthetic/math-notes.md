# 機器學習筆記：線性迴歸

## 模型

給定資料集 $\{(x_i, y_i)\}_{i=1}^n$，線性迴歸假設

$$
\hat{y} = w^\top x + b
$$

其中 $w \in \mathbb{R}^d$ 是權重，$b$ 是偏差。

## 損失函數

均方誤差（MSE）定義為：

$$
L(w, b) = \frac{1}{n} \sum_{i=1}^{n} (y_i - w^\top x_i - b)^2
$$

注意 $x_i^*$ 與 $y_*$ 的星號在數學式裡不算強調，$a_1 * b_2 * c_3$ 也一樣。

## 閉式解

令 $X$ 為設計矩陣，則
$$w^* = (X^\top X)^{-1} X^\top y$$

**條件：**$X^\top X$ 必須可逆。

## 梯度下降

更新規則：

$$
\begin{aligned}
w &\leftarrow w - \eta \nabla_w L \\
b &\leftarrow b - \eta \frac{\partial L}{\partial b}
\end{aligned}
$$

學習率 $\eta$ 太大會發散，太小會收斂很慢。實務上常用 $\eta = 10^{-3}$。

> 引言裡的數學式：
> $$
> \nabla_w L = -\frac{2}{n} X^\top (y - Xw)
> $$

- 清單裡的數學式：
  $$
  R^2 = 1 - \frac{SS_{res}}{SS_{tot}}
  $$

## 費用估算

GPU 每小時 $2.5，跑 40 小時約 $100。這裡的金額不是數學式，**總計 $100 ** 左右。

## 參考資料
- Bishop, *Pattern Recognition and Machine Learning*, 2006
- https://en.wikipedia.org/wiki/Linear_regression
