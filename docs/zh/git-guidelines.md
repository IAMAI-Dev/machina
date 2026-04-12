# Machina Git 指南

Machina 项目的 Git 提交和 Pull Request 约定。

## 提交消息

### 格式

```
module: subject

Body describing what changed and why.

Signed-off-by: Name <email>
```

### Subject 行

- 格式：`module: subject`
- 祈使语气：`add`、`fix`、`remove`——而非 `added`、`fixed`、`removed`
- 小写 subject，不加句号
- 总长度不超过 72 字符

### 常用 module 前缀

| Module | 范围 |
|--------|------|
| `core` | IR 类型、操作码、CPU trait |
| `accel` | IR 优化、寄存器分配、代码生成、执行引擎 |
| `guest/riscv` | RISC-V 前端（解码、翻译） |
| `decode` | .decode 解析器和代码生成 |
| `system` | CPU 管理器、GDB 桥接、WFI |
| `memory` | AddressSpace、MMIO、RAM 块 |
| `hw/core` | 设备基础设施（qdev、IRQ、FDT） |
| `hw/intc` | PLIC、ACLINT |
| `hw/char` | UART |
| `hw/riscv` | 参考机器、启动、SBI |
| `hw/virtio` | VirtIO MMIO 传输和设备 |
| `monitor` | QMP/HMP 控制台 |
| `gdbstub` | GDB 远程协议 |
| `difftest` | 差分测试客户端 |
| `tests` | 测试套件 |
| `docs` | 文档 |
| `project` | 跨模块变更（CI、Makefile、配置） |

### 常用动词前缀

| 动词 | 用途 |
|------|------|
| `Fix` | 修复缺陷 |
| `Add` | 引入新功能 |
| `Remove` | 删除代码或功能 |
| `Refactor` | 不改变行为的重构 |
| `Rename` | 更改文件、模块或符号名称 |
| `Implement` | 添加新的子系统或功能 |
| `Enable` | 启用之前禁用的功能 |
| `Clean up` | 无功能变更的小清理 |
| `Bump` | 更新依赖版本 |

### Body

- 与 subject 之间空一行
- 描述修改了什么以及为什么——而非如何实现
- 每行不超过 80 字符

### 示例

```
accel: fix register clobber in div/rem helpers

The x86-64 backend used RDX as a scratch register for division
without saving the guest's original value. Add save/restore around
the DIV instruction.

Signed-off-by: Chao Liu <chao.liu.zevorn@gmail.com>
```

```
guest/riscv: implement Zbs (single-bit operations)

Add bclr, bset, binv, bext for both register and immediate forms.
The decoder now recognizes the Zbs extension when enabled in
misa.

Signed-off-by: Chao Liu <chao.liu.zevorn@gmail.com>
```

## 原子提交

### 一个提交只做一件事

每个提交必须只做一个逻辑变更。不要在单个提交中混合不相关的变更。
如果你在提交消息中写了"以及"，就应该拆分。

### 每个提交必须编译通过且测试通过

每个提交之后，代码树必须处于可工作状态。禁止有破坏性的中间状态。
这确保 `git bisect` 始终有效。

### 提交 PR 前合并临时补丁

在提交 PR 之前审查自己的分支时，将所有临时提交合并到它们所属的
提交中。临时提交包括：

- 修正早期提交中引入的拼写错误或缺陷的修复补丁
- 调整先前变更的补丁（如重命名、重新排序）
- 任何消息以 `fixup!` 或 `squash!` 开头的提交

使用 `git rebase -i` 将这些临时提交折叠到原始提交中。最终 PR 的
历史应该是一组干净的逻辑变更序列，而不是开发日记。

### 重构与功能分离

如果功能需要预备性的重构，将重构放在独立的提交中，在功能提交之前。
这使得每个提交更容易审查和二分定位。

## Signed-off-by

本仓库的所有提交必须包含 `Signed-off-by` 行：

```
Signed-off-by: Chao Liu <chao.liu.zevorn@gmail.com>
```

禁止添加 AI 相关的签名行（如 `Co-Authored-By: Claude`）。

## Pull Request

### 保持 PR 专注

一个 PR 一个主题。混合了缺陷修复、重构和新功能的 PR 难以审查。

### CI 必须通过

请求 review 前确保所有 CI 检查通过：

- `make test`——所有测试通过
- `make clippy`——零警告
- `make fmt-check`——格式正确

### 引用 issue

当 PR 解决某个 issue 时，在描述中引用：

```
Closes #42
```
