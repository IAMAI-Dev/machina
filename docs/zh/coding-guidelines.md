# Machina 编码指南

Machina 项目的通用编码指南。Rust 特定规则见 [Rust 指南](rust-guidelines.md)，
格式和命名约定见 [代码风格](coding-style.md)。

## 命名

### 描述性命名

禁止单字母命名或含糊缩写。名称应揭示意图。

```rust
// Bad
let v = t.read();
let p = addr >> 12;

// Good
let value = temp.read();
let page_number = addr >> PAGE_SHIFT;
```

### 准确命名

名称必须与代码的实际行为一致。如果名称是"count"，值就应该是计数——
而不是索引、偏移量或掩码。

### 名称中编码单位

当值携带物理单位或量级时，将其嵌入名称。

```rust
let timeout_ms = 5000;
let frame_size_in_pages = 4;
let clock_freq_hz = 12_000_000;
```

### 布尔值命名

使用断言式命名：`is_*`、`has_*`、`can_*`、`should_*`。

```rust
let is_kernel_mode = mode == Prv::M;
let has_side_effects = op.flags().contains(OpFlags::SIDE_EFFECT);
```

## 注释

### 解释为什么，而不是什么

重复代码的注释是噪音。解释非显而易见决策背后的原因。

```rust
// Bad: 重复代码
// Check if page is present
if pte.flags().contains(PteFlags::V) { ... }

// Good: 解释约束
// Sv39 spec: fetch falls through when V=0 in S-mode
// only traps in M-mode (spec 4.3.1)
if pte.flags().contains(PteFlags::V) { ... }
```

### 记录设计决策

当存在多种方案时，记录选择当前方案的原因。未来的读者需要理解权衡，
而不仅仅是结果。

### 引用规范

实现硬件行为时，引用规范章节。

```rust
// RISC-V Privileged Spec 4.3.1 -- PTE attribute for global mappings
const PTE_G: u64 = 0x10;
```

## 文件组织

### 每个文件一个概念

当文件过长或混合了不相关的职责时，拆分文件。名为 `mmu.rs` 的文件
不应包含中断处理逻辑。

### 自上而下的阅读顺序

高层入口点放在前面。辅助函数和内部细节放在后面。读者应能通过阅读
第一个部分来理解公共 API。

### 分组为逻辑段落

在函数内部，将相关语句分组在一起。用空行分隔不同的组。每组应表达
算法中的一个步骤。

## API 设计

### 隐藏实现细节

默认使用最窄的可见性。只暴露调用者需要的内容。

```rust
// Prefer
pub(crate) fn translate_one(ctx: &mut Context) { ... }

// Avoid
pub fn translate_one(ctx: &mut Context) { ... }
```

### 边界验证，内部信任

在公共 API 边界（如 syscall 入口、设备 MMIO 写入）验证输入。
在 crate 内部，信任已验证的值。

### 用类型强制不变量

如果值有约束，用类型系统编码，而不是在每个使用点检查。

```rust
// Prefer: 非法状态不可表示
pub struct PhysicalPage(u64);

impl PhysicalPage {
    pub fn new(frame: u64) -> Option<Self> {
        (frame < MAX_PHYS_PAGE).then_some(PhysicalPage(frame))
    }
}

// Avoid: 原始 u64 可以是任何值
fn map_page(frame: u64) { ... }
```

## 错误消息

一致地格式化错误消息。包含失败的操作、涉及的值或标识符，以及
（适用时）期望的范围。

```
"invalid PTE at {vpn}: reserved bits set"
"out of TB cache: capacity {cap}, requested {size}"
```
