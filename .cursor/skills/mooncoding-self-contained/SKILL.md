---
name: mooncoding-self-contained
description: >-
  MoonCoding is a product-deploy project: features must be implemented in-repo
  and flexibly deployable as a self-contained payload. Use when designing board
  features, networking, packaging, dependencies, install steps, or when tempted
  to rely on fixed NIC names, board-side apt/opkg downloads, or host-only tools.
---

# 产品自包含部署（必守）

本项目是**面向产品部署**的项目，不是实验室一次性环境。

## 原则

1. **功能只能在项目里实现**，并保证可灵活部署（交叉编好 → stage → adb/脚本推到板端即可跑）。
2. **不能依赖固定的第三方特性**，例如：
   - 写死某块网卡名 / 某条 `wlan0` / 某厂商驱动路径
   - 要求板端现场 `apt` / `opkg` / 手工下载第三方包才能用
   - 依赖开发机 PATH 里碰巧有的工具、未随产品打包的运行时
3. **一切以自包含形式交付**：运行时库、资源、字体、进程助手、配置默认值，能进产品包的就进包；板端只接收部署产物，不「再去网上装依赖」。

## 做功能时的检查

- 新依赖：能否打进 `build-board` / `qt6-stage` / 应用目录随部署带走？
- 硬件相关：是否用探测/枚举/配置，而不是写死设备节点？
- 文档/脚本：是否在教用户「板上再装某某包」？若是 → 改成项目内打包或可选降级，而不是甩给现场。

## 反例 → 正例

| 反例 | 正例 |
|------|------|
| 板端 `opkg install foo` 才能预览 HTML | 把 WebEngine/libs/resources 打进 stage 一并 adb push |
| 代码写死 `wlan0` | 枚举接口 / 读配置 / 用系统默认路由 |
| 依赖开发机全局 `adb` | 项目 skill + 部署脚本写死本机工具路径候选 |
