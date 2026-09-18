# 数据设计

## 当前结构

数据库保留 app_local_data_dir/database/app.sqlite；图片保留 workspaces/<workspace-id>/originals/<image-id>.<extension>；临时文件位于 temp。本轮不迁移用户数据。

版本 1 SQL 提取至 src-tauri/migrations/001_initial.sql，语义和 user_version 不变，已有数据库不重复执行迁移。保留 workspaces/images/ocr_runs、外键与 WAL。

当前图片元数据与 pending run 在 documents repository 原子入库。下一步拆为 image_assets 独立图片入库和 ocr 独立创建 run，不建立同用途重复表。

## 后续版本 2（尚未执行）

| 表或扩展 | 字段与约束 |
| --- | --- |
| images 扩展 | source：import/clipboard/capture |
| pins | id、workspace_id、image_id、zoom、is_open、created_at、updated_at；zoom > 0，图片外键 RESTRICT |
| notes | id、workspace_id、text、color、revision、is_open、created_at、updated_at；revision >= 0 |
| window_states | object_kind + object_id 主键，monitor_id、x、y、width、height、always_on_top；尺寸 > 0 |
| ocr_runs 扩展 | queued/running/completed/failed/cancelled、revision；保留 engine_version |
| ocr_blocks | run_id + block_index 主键，text、confidence、polygon_json；删除 run 时级联 |
| settings | key 主键，value_json、updated_at |

版本 1 不修改，后续用有序事务迁移；数据库版本高于应用支持版本时明确拒绝。window_states 多态归属由应用事务维护，不存在跨 pins/notes 的 SQL 外键。删除对象时同事务删除状态，状态更新先检查对象仍存在。

窗口使用逻辑坐标，OCR polygon 使用原图物理像素。贴图穿透不持久化。notes revision 使用条件 UPDATE，冲突不覆盖。

## 资源生命周期

导入先写临时文件并校验，移动到最终路径后插入元数据，入库失败清理新文件。OCR 仅引用已存在 imageId，失败保留图片和失败记录。

被 pins 引用的图片不可删除。工作区删除先处理窗口和任务，再在明确操作中删除所属对象。关闭窗口不删除图片。

复用现有删除暂存机制：文件移到 temp → 数据库事务删除 → 清理暂存；事务失败恢复原文件是数据一致性处理，不是备用实现。清理失败明确报错。

启动恢复 is_open 的 pins/notes，校正到当前显示器。截图和 OCR 结果窗口不恢复。遗留 queued/running OCR 标记 failed 并记录进程中断，不自动重试推理。

迁移验证覆盖旧版本升级、重复启动、引用约束和事务失败清理。
