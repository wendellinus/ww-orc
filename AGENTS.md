# 项目开发约束

## 遵循对应版本的官方规范

- 编写或修改代码前，先从依赖声明和锁文件确认相关框架、库的实际版本，遵循该版本的官方文档及推荐规范。不得直接套用旧教程、旧模板或记忆中的旧写法，也不得盲目使用项目版本尚不支持的新语法。
- 遇到不确定的 API、类名或版本差异时，查阅对应版本的官方文档；必要时核对官方源码，确认后再实现。

## Tailwind CSS

- 当前锁文件使用 Tailwind CSS 4.3.3；后续以项目实际依赖和锁文件为准。
- 新增或修改 Tailwind 类名时，必须使用该版本支持的官方推荐规范形式（canonical classes），避免旧别名和已弃用写法。
- 本项目使用 `wrap-break-word`，不再新增 `break-words`。其他类名也应按对应版本核对，不能只修正这一例。
- 遇到 `suggestCanonicalClasses` 提示时，核对官方推荐并修正代码，不应把处理自己生成的旧写法留给用户。
- 交付前检查本次新增或修改的 Tailwind 类名，清理旧别名及不规范写法；有可用的语言服务诊断时一并验证。构建成功不等同于通过编辑器的规范类名检查；无法运行诊断时如实说明，不得声称已通过。
- 除非用户明确要求调整检查配置，不得通过关闭 `tailwindCSS.lint.suggestCanonicalClasses`、禁用校验或压制提示来掩盖代码问题。

官方参考：
- Tailwind CSS 文档：https://tailwindcss.com/docs
- overflow-wrap：https://tailwindcss.com/docs/overflow-wrap
- IntelliSense 规范类名检查：https://github.com/tailwindlabs/tailwindcss-intellisense#tailwindcsslintsuggestcanonicalclasses
