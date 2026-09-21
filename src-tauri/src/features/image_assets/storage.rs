use image::{GenericImageView, ImageFormat, ImageReader};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

use super::types::StoredImage;

const MAX_IMAGE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_CLIPBOARD_IMAGE_BYTES: usize = 20 * 1024 * 1024;

pub struct AppStorage {
    root: PathBuf,
}

impl AppStorage {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("workspaces"))
            .map_err(|error| format!("创建工作区图片目录失败: {error}"))?;
        fs::create_dir_all(root.join("temp"))
            .map_err(|error| format!("创建临时目录失败: {error}"))?;
        Ok(Self { root })
    }

    pub fn capture_path(&self, session: &str, monitor: u32) -> PathBuf {
        self.root
            .join("temp")
            .join(format!("capture-{session}-{monitor}.png"))
    }

    pub fn store_rgba(
        &self,
        workspace: &str,
        image: &image::RgbaImage,
    ) -> Result<StoredImage, String> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut buffer, ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let mut stored = self.store_bytes(workspace, buffer.get_ref())?;
        stored.original_name = "截图.png".into();
        Ok(stored)
    }

    pub fn store_path(&self, workspace_id: &str, source: &Path) -> Result<StoredImage, String> {
        validate_workspace_id(workspace_id)?;
        if !source.is_file() {
            return Err(format!("图片文件不存在: {}", source.display()));
        }

        let byte_size = source
            .metadata()
            .map_err(|error| format!("读取图片信息失败: {error}"))?
            .len();
        if byte_size > MAX_IMAGE_BYTES {
            return Err("图片不能超过 100 MB".to_string());
        }

        let reader = ImageReader::open(source)
            .map_err(|error| format!("打开图片失败: {error}"))?
            .with_guessed_format()
            .map_err(|error| format!("识别图片格式失败: {error}"))?;
        let format = reader
            .format()
            .ok_or_else(|| "无法识别图片格式".to_string())?;
        let (extension, mime_type) = supported_format(format)?;
        let (width, height) = reader
            .into_dimensions()
            .map_err(|error| format!("读取图片尺寸失败: {error}"))?;

        let original_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("image")
            .to_string();

        self.persist(
            workspace_id,
            &original_name,
            extension,
            mime_type,
            byte_size as i64,
            width,
            height,
            |temporary_path| {
                fs::copy(source, temporary_path)
                    .map(|_| ())
                    .map_err(|error| format!("复制图片失败: {error}"))
            },
        )
    }

    pub fn store_bytes(&self, workspace_id: &str, bytes: &[u8]) -> Result<StoredImage, String> {
        validate_workspace_id(workspace_id)?;
        if bytes.is_empty() {
            return Err("剪贴板图片内容为空".to_string());
        }
        if bytes.len() > MAX_CLIPBOARD_IMAGE_BYTES {
            return Err("剪贴板图片不能超过 20 MB".to_string());
        }

        let format =
            image::guess_format(bytes).map_err(|error| format!("无法读取剪贴板图片: {error}"))?;
        let (extension, mime_type) = supported_format(format)?;
        let decoded = image::load_from_memory_with_format(bytes, format)
            .map_err(|error| format!("解析剪贴板图片失败: {error}"))?;
        let (width, height) = decoded.dimensions();

        self.persist(
            workspace_id,
            &format!("剪贴板图片.{extension}"),
            extension,
            mime_type,
            bytes.len() as i64,
            width,
            height,
            |temporary_path| {
                fs::write(temporary_path, bytes)
                    .map_err(|error| format!("写入剪贴板图片失败: {error}"))
            },
        )
    }

    pub fn absolute_path(&self, relative_path: &str) -> Result<PathBuf, String> {
        let relative = Path::new(relative_path);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err("数据库中的图片路径无效".to_string());
        }
        Ok(self.root.join(relative))
    }

    pub fn remove_file(&self, path: &Path) {
        let _ = fs::remove_file(path);
    }

    pub fn stage_document_deletion(
        &self,
        relative_path: &str,
    ) -> Result<Option<StagedDeletion>, String> {
        let path = self.absolute_path(relative_path)?;
        self.stage_deletion(path)
    }

    pub fn stage_workspace_deletion(
        &self,
        workspace_id: &str,
    ) -> Result<Option<StagedDeletion>, String> {
        validate_workspace_id(workspace_id)?;
        self.stage_deletion(self.root.join("workspaces").join(workspace_id))
    }

    fn persist(
        &self,
        workspace_id: &str,
        original_name: &str,
        extension: &str,
        mime_type: &str,
        byte_size: i64,
        width: u32,
        height: u32,
        write: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<StoredImage, String> {
        let id = Uuid::new_v4().to_string();
        let relative_path = PathBuf::from("workspaces")
            .join(workspace_id)
            .join("originals")
            .join(format!("{id}.{extension}"));
        let absolute_path = self.root.join(&relative_path);
        let parent = absolute_path
            .parent()
            .ok_or_else(|| "无法确定图片目录".to_string())?;
        fs::create_dir_all(parent).map_err(|error| format!("创建图片目录失败: {error}"))?;

        let temporary_path = self
            .root
            .join("temp")
            .join(format!("import-{id}.{extension}"));
        write(&temporary_path)?;
        if let Err(error) = fs::rename(&temporary_path, &absolute_path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(format!("保存图片失败: {error}"));
        }

        Ok(StoredImage {
            id,
            workspace_id: workspace_id.to_string(),
            original_name: original_name.to_string(),
            relative_path: relative_path.to_string_lossy().replace('\\', "/"),
            absolute_path,
            mime_type: mime_type.to_string(),
            byte_size,
            width: i64::from(width),
            height: i64::from(height),
        })
    }

    fn stage_deletion(&self, original: PathBuf) -> Result<Option<StagedDeletion>, String> {
        if !original.exists() {
            return Ok(None);
        }

        let staged = self
            .root
            .join("temp")
            .join(format!("delete-{}", Uuid::new_v4()));
        fs::rename(&original, &staged).map_err(|error| format!("暂存待删除文件失败: {error}"))?;

        Ok(Some(StagedDeletion { original, staged }))
    }
}

pub struct StagedDeletion {
    original: PathBuf,
    staged: PathBuf,
}

impl StagedDeletion {
    pub fn restore(self) -> Result<(), String> {
        if let Some(parent) = self.original.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("恢复文件目录失败: {error}"))?;
        }
        fs::rename(self.staged, self.original)
            .map_err(|error| format!("恢复待删除文件失败: {error}"))
    }

    pub fn finalize(self) -> Result<(), String> {
        if self.staged.is_dir() {
            fs::remove_dir_all(self.staged).map_err(|error| format!("删除工作区文件失败: {error}"))
        } else {
            fs::remove_file(self.staged).map_err(|error| format!("删除图片文件失败: {error}"))
        }
    }
}

fn supported_format(format: ImageFormat) -> Result<(&'static str, &'static str), String> {
    match format {
        ImageFormat::Png => Ok(("png", "image/png")),
        ImageFormat::Jpeg => Ok(("jpg", "image/jpeg")),
        _ => Err("仅支持 PNG、JPG 或 JPEG 图片".to_string()),
    }
}

fn validate_workspace_id(workspace_id: &str) -> Result<(), String> {
    if workspace_id == "default" || Uuid::parse_str(workspace_id).is_ok() {
        Ok(())
    } else {
        Err("工作区 ID 无效".to_string())
    }
}
