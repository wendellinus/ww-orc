# 数据设计

数据库位于 app_local_data_dir/database/app.sqlite；图片位于 workspaces/<workspace-id>/originals/<image-id>.<extension>；截图预览及删除暂存位于 temp。已有数据不搬迁。

## 已执行的 schema

版本 1：001_initial.sql，workspaces/images/ocr_runs，保持原始结构。

版本 2：002_desktop.sql，新增 notes/pins/window_states，以事务升级，重复启动幂等。高于应用支持版本的数据库明确拒绝。

版本 3：003_ocr_blocks.sql，为每次 OCR 保存文字块文本及原图像素坐标；旧记录默认为空数组。

| 表 | 归属与约束 |
| --- | --- |
| images | 原图元数据；截图可以独立入库，不必创建 OCR 记录 |
| ocr_runs | 每次识别单独记录，引用已有图像；保存纯文本及文字块坐标；pending/completed/failed |
| notes | 旧版便签兼容表；当前应用不再读写 |
| pins | image_id、zoom、is_open、时间；图片外键 RESTRICT；zoom 0.1 至 5 |
| window_states | object_kind + object_id 主键；物理像素位置和尺寸、topmost |
| app_settings | Rust 启动阶段所需设置；当前保存原生快捷键和活动工作区 |

历史 notes 表不主动删除，避免数据库升级破坏旧数据。图片像素、OCR 输入和窗口物理尺寸保持明确含义；前端选区通过屏幕比例映射原图，图片 100% 显示时按窗口 devicePixelRatio 转换 CSS 尺寸。

window_states 由窗口基础模块维护，对象删除与状态删除在同一事务完成。恢复时检查当前显示器，原位置不再可访问则使用居中位置。穿透尚未提供。

## 生命周期

文件导入先写临时文件，移动到最终路径，再入库。入库失败清理新文件。OCR 失败保留图片及失败记录。已有图像的新 OCR run 不重复插入 images。

贴图引用阻止原图删除，删除检查发生在文件暂存之前。工作区含贴图时拒绝直接删除，提示先删除所属对象。关闭窗口不删除对象和图片。

关闭对象窗口保存最终几何状态并设置 is_open=false；退出应用保存贴图几何状态，但保持 is_open，使下次启动恢复这些对象。退出过程中 Destroyed 事件不修改恢复标记。

删除仍使用现有文件暂存机制保证数据库失败时能恢复文件，清理失败明确报告。

验证覆盖版本升级、幂等迁移和图片引用约束。
