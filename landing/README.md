# Epistemic Landing

| 路由 | 内容 |
|------|------|
| `/` | 产品官网（简洁液态玻璃） |
| `/report` | 项目汇报 PPT（整页上下滑动） |

## 视觉

- **液态玻璃**：共享 `Glass` 组件（CSS `backdrop-blur` + 高光，可大量使用且不卡）
- **React Bits（克制）**
  - 官网：`SoftAurora`（单层背景）· `BlurText` · `GradientText` · `FadeContent` · `SpotlightCard` · `Magnet`
  - 汇报：`SoftAurora`（整页一层氛围）· `BlurText` · `CountUp` · `FadeContent` + 玻璃幻灯片框
- 不做多 WebGL 叠层（避免卡顿）

## 开发

```bash
cd landing
npm install
npm run dev
# http://localhost:5174/
# http://localhost:5174/report
```

### 汇报操作

- 滚轮 / 触控板 / 触屏上下翻页（snap）
- `↑↓` `←→` 空格 `j/k` · `Home/End` · `F` 全屏

## 构建

```bash
npm run build
```
