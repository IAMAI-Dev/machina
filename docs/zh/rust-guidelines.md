# Machina Rust 指南

Machina 项目的 Rust 特定指南。通用编码规则见 [编码指南](coding-guidelines.md)，
格式约定见 [代码风格](coding-style.md)。

## Unsafe Rust

### 每次使用 unsafe 都需要理由

每个 `unsafe` 块都需要 `// SAFETY:` 注释解释操作为何安全。
每个 `unsafe fn` 或 `unsafe trait` 都需要 `# Safety` 文档段落，
描述调用者必须满足的条件。

```rust
// SAFETY: buf points to a valid, RWX-mapped region of `len` bytes.
// The mmap call above guaranteed alignment and permissions.
unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), buf, len) }
```

### unsafe 仅限于特定模块

`unsafe` 仅允许用于 JIT 代码缓冲区管理、生成代码的函数指针转换、
TLB 快速路径中的原始指针访问、后端发射器中的内联汇编和 FFI。
所有其他代码必须是安全 Rust。

## 函数

### 保持函数小而专注

一个函数做一件事。如果需要注释来分隔段落，考虑拆分。

### 最小化嵌套

目标最多 3 层嵌套。使用提前返回、`let...else` 和 `?` 来展平控制流。

```rust
// Prefer
let Some(pte) = page_table.walk(vpn) else {
    return Err(MmuFault::InvalidPte);
};

// Avoid
if let Some(pte) = page_table.walk(vpn) {
    // ... deeply nested logic ...
}
```

### 避免布尔参数

使用枚举或拆分为两个函数。

```rust
// Prefer
pub fn emit_load_signed(buf: &mut CodeBuffer, ...) { ... }
pub fn emit_load_unsigned(buf: &mut CodeBuffer, ...) { ... }

// Avoid
pub fn emit_load(buf: &mut CodeBuffer, signed: bool, ...) { ... }
```

## 类型与 Trait

### 封闭集合优先用枚举

当变体集合在编译期已知时，使用枚举。trait 对象（`dyn Trait`）仅适用于
开放式扩展。

```rust
// Prefer
enum Exception {
    InstructionAccessFault,
    LoadAccessFault,
    StoreAccessFault,
    // ...
}

// Avoid
trait Exception { fn handle(&self); }
```

### 通过 getter 封装字段

通过方法暴露字段而非直接 `pub`。这保留了后续添加验证或日志的能力。

## 模块与 Crate

### 默认使用最窄可见性

默认使用 `pub(super)` 或 `pub(crate)`。仅在外部 crate 确实需要访问时
才使用 `pub`。

### 通过父模块限定函数导入

导入父模块，然后通过它调用函数。这使来源更明确。

```rust
// Prefer
use core::mem;
mem::replace(&mut slot, new_value)

// Avoid
use core::mem::replace;
replace(&mut slot, new_value)
```

### 使用 workspace 依赖

所有共享依赖版本必须在 workspace 根 `Cargo.toml` 的
`[workspace.dependencies]` 中声明。各 crate 使用 `{ workspace = true }`
引用。

## 错误处理

### 用 `?` 传播错误

在可能失败的地方不要 `.unwrap()` 或 `.expect()`。使用 `?` 将错误
传播给调用者。

### 定义领域错误类型

使用专用的错误枚举而非泛型 `String` 或 `Box<dyn Error>`。

```rust
#[derive(Debug)]
enum TranslateError {
    InvalidOpcode(u32),
    UnsupportedExtension(char),
    BufferOverflow { requested: usize, available: usize },
}
```

## 并发

### 记录锁顺序

当存在多个锁时，文档化获取顺序并始终一致地遵守，以防止死锁。

### 自旋锁下不做 I/O

持有自旋锁时绝不执行 I/O 或阻塞操作。这包括内存分配和打印语句。

### 避免随意使用原子操作

`Ordering` 非常微妙。默认使用 `SeqCst`。仅在具有文档化的性能原因
且正确性论证已写入注释时，才放宽为 `Acquire`/`Release`/`Relaxed`。

## 性能

### 热路径禁止 O(n)

翻译快速路径（TB 查找、代码执行、TLB 遍历）必须避免 O(n) 操作。
使用哈希表、直接映射缓存或索引数组。

### 最小化不必要的拷贝

通过引用传递大型结构体。当不需要所有权时使用 `&[u8]` 而非 `Vec<u8>`。
避免在每次迭代中克隆 `Arc`。

### 不要过早优化

优化提交必须包含显示改进的基准测试。

## 宏与属性

### 函数优先于宏

仅在函数无法完成工作时使用宏（例如重复声明、生成 match 分支）。

### 在最窄范围抑制 lint

将 `#[allow(...)]` 或 `#[expect(...)]` 应用于特定项，而非整个模块。

### derive trait 按字母排序

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
```

### Workspace lints

每个 workspace 成员必须包含：

```toml
[lints]
workspace = true
```
