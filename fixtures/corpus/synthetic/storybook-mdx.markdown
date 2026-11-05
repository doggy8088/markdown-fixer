import { Meta, Canvas, Story, ArgsTable } from '@storybook/blocks';
import * as ButtonStories from './Button.stories';

<Meta of={ButtonStories} />

# Button 按鈕元件

按鈕用來觸發動作。設計規範請見 [Figma](https://www.figma.com/file/abc123/Design-System)。

<Canvas of={ButtonStories.Primary} />

## 何時使用
- 送出表單
- 開啟對話框
- 觸發**  主要動作 **

## 變體

<Canvas>
  <Story of={ButtonStories.Secondary} />
  <Story of={ButtonStories.Danger} />
</Canvas>

| 變體 | 用途 |
| --- | --- |
| `primary` | 頁面上最重要的動作，一頁最多一個 |
| `secondary` | 次要動作 |
| `danger` | 刪除等不可逆的動作 |

## Props

<ArgsTable of={ButtonStories} />

## 無障礙
1.按鈕文字要能說明動作，不要只寫「點這裡」
2. 只有圖示的按鈕要加 `aria-label`
3. 不要用 `<div onClick>` 假裝按鈕

{/* MDX 2 的註解寫法 */}

export const Example = () => (
  <Button variant="primary" onClick={() => alert('#clicked')}>
    送出
  </Button>
);
