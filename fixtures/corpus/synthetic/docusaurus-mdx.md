---
id: getting-started
title: 快速開始
sidebar_position: 1
description: 五分鐘內在本機跑起來
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';
import { Highlight } from '@site/src/components/Highlight';

# 快速開始

<Highlight color="#25c2a0">本頁約需 5 分鐘</Highlight>，帶你安裝 CLI 並建立第一個專案。

:::tip 小提示
已經裝過舊版的話，請先執行 `npm uninstall -g example-cli`。
:::

## 安裝

<Tabs groupId="os">
  <TabItem value="mac" label="macOS" default>

```bash
brew install example-cli
```

  </TabItem>
  <TabItem value="win" label="Windows">

```powershell
winget install Example.Cli
```

  </TabItem>
</Tabs>

## 建立專案

```bash
example-cli init my-app
cd my-app
example-cli dev
```

打開 http://localhost:3000 就能看到預設頁面。

:::warning
`example-cli dev` 會監看檔案變更，**不要** 在 `node_modules` 很大的目錄下執行。
:::

## 專案結構

```text
my-app/
├── docs/
├── src/
│   └── pages/
└── docusaurus.config.js
```

<details>
  <summary>為什麼有兩個設定檔？</summary>

`docusaurus.config.js` 是網站設定，`sidebars.js` 只管側邊欄。分開之後，改側邊欄不用重新載入整個設定。

</details>

export const Version = () => <code>v{process.env.VERSION}</code>;

目前版本：<Version />

## 下一步
- 閱讀[設定檔說明](./configuration.md)
- 了解 [MDX 語法](https://mdxjs.com/docs/what-is-mdx/)
- 部署到 GitHub Pages
