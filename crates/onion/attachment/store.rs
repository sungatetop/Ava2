//! 附件内容寻址存储。
//!
//! 关键不变量：
//! - id 恒为 `sha256:<64hex>`，对象文件名取裸 hex（`:` 非法于 Windows 文件名）；
//! - staged 写：tmp 唯一文件 → fsync → hard-link 到目标 → `AlreadyExists` 时
//!   读回复核摘要（去重），随后剥离 tmp 名并置只读权限；
//! - 读路径恒校验：摘要不符 / 字节数不符 / 元数据（媒体类型、尺寸）不符
//!   一律 `ATTACHMENT_CORRUPT`。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{AttachmentError, FileAttachmentRef, ImageAttachmentRef};

/// 图片对象桶。
const OBJECTS: &str = "objects";
/// 文件 canonical 对象桶。
const FILE_OBJECTS: &str = "file-objects";
/// 文件别名桶：`files/<2hex>/<hex>/<叶名>`。
const FILES: &str = "files";
/// staged 写暂存目录。
const STAGING: &str = "tmp";

/// 待入库图片（已解码字节；准入限额由 [`super::admission`] 把关）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveImage {
    pub data: Vec<u8>,
    pub media_type: String,
    pub name: Option<String>,
}

/// 待入库文件（字节原样存储；空文件合法）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFile {
    pub data: Vec<u8>,
    pub name: Option<String>,
}

/// 附件存储句柄：根目录下布局 `objects/ | file-objects/ | files/ | tmp/`。
pub struct AttachmentStore {
    root: PathBuf,
}

impl AttachmentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 嗅探图片入库：非空 + 可识别 + 声明媒体类型一致 → 内容寻址发布。
    pub fn save_image(&self, input: SaveImage) -> Result<ImageAttachmentRef, AttachmentError> {
        let (media_type, width, height) = detect_image(&input.data)?;
        if media_type != input.media_type {
            return Err(AttachmentError::new(
                "IMAGE_TYPE_MISMATCH",
                format!(
                    "declared media type {} does not match detected {}",
                    input.media_type, media_type
                ),
            ));
        }
        let hex = sha256_hex(&input.data);
        self.publish_object(OBJECTS, &hex, &input.data)?;
        Ok(ImageAttachmentRef {
            attachment_id: format!("sha256:{hex}"),
            media_type,
            bytes: input.data.len() as u64,
            width,
            height,
            name: display_name(input.name.as_deref()),
        })
    }

    /// 文件入库：字节原样 + 叶名别名（同名同内容幂等）。
    pub fn save_file(&self, input: SaveFile) -> Result<FileAttachmentRef, AttachmentError> {
        let hex = sha256_hex(&input.data);
        self.publish_object(FILE_OBJECTS, &hex, &input.data)?;
        let leaf = file_leaf_name(input.name.as_deref());
        let alias_dir = self.root.join(FILES).join(&hex[..2]).join(&hex);
        let alias = alias_dir.join(&leaf);
        fs::create_dir_all(&alias_dir).map_err(write_failed(&alias_dir))?;
        if !alias.exists() {
            let object = self.root.join(FILE_OBJECTS).join(&hex[..2]).join(&hex);
            fs::hard_link(&object, &alias).map_err(write_failed(&alias))?;
        }
        Ok(FileAttachmentRef {
            attachment_id: format!("sha256:{hex}"),
            name: leaf,
            bytes: input.data.len() as u64,
        })
    }

    /// 读图并全量复核（摘要 + 字节数 + 媒体类型 + 尺寸）。
    pub fn read_image(&self, reference: &ImageAttachmentRef) -> Result<Vec<u8>, AttachmentError> {
        let hex = parse_attachment_id(&reference.attachment_id)?;
        let data = read_object(&self.root.join(OBJECTS).join(&hex[..2]).join(&hex))?;
        verify_image(&data, reference, &hex)?;
        Ok(data)
    }

    /// 读文件并复核（摘要 + 字节数）。
    pub fn read_file(&self, reference: &FileAttachmentRef) -> Result<Vec<u8>, AttachmentError> {
        let hex = parse_attachment_id(&reference.attachment_id)?;
        let data = read_object(&self.root.join(FILE_OBJECTS).join(&hex[..2]).join(&hex))?;
        verify_bytes(&data, reference, &hex)?;
        Ok(data)
    }

    /// 发布不可变对象：已存在则摘要复核去重；否则 staged 写 + hard-link。
    fn publish_object(
        &self,
        bucket: &str,
        hex: &str,
        data: &[u8],
    ) -> Result<(), AttachmentError> {
        let target = self.root.join(bucket).join(&hex[..2]).join(hex);
        if target.exists() {
            let existing = read_object(&target)?;
            if sha256_hex(&existing) != hex {
                return Err(corrupt(&target));
            }
            return Ok(());
        }
        let parent = target.parent().ok_or_else(|| {
            AttachmentError::new(
                "ATTACHMENT_WRITE_FAILED",
                format!("invalid object path {}", target.display()),
            )
        })?;
        fs::create_dir_all(parent).map_err(write_failed(parent))?;
        let staging_dir = self.root.join(STAGING);
        fs::create_dir_all(&staging_dir).map_err(write_failed(&staging_dir))?;
        let tmp = staging_dir.join(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
        staged_write(&tmp, data)?;
        match fs::hard_link(&tmp, &target) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = read_object(&target)?;
                if sha256_hex(&existing) != hex {
                    let _ = fs::remove_file(&tmp);
                    return Err(corrupt(&target));
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&tmp);
                return Err(write_failed(&tmp)(error));
            }
        }
        // 剥离暂存名（best-effort：目标已发布，残留仅是 tmp 垃圾）。
        let _ = fs::remove_file(&tmp);
        make_readonly(&target)?;
        sync_dir(parent);
        Ok(())
    }
}

/// staged 单文件写：`create_new`（0600 / Windows 默认）→ 全量写 → fsync。
fn staged_write(path: &Path, data: &[u8]) -> Result<(), AttachmentError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(write_failed(path))?;
    file.write_all(data).map_err(write_failed(path))?;
    file.sync_all().map_err(write_failed(path))
}

/// 置只读（POSIX chmod 0400 / Windows readonly 位），失败即写故障。
fn make_readonly(path: &Path) -> Result<(), AttachmentError> {
    let metadata = fs::metadata(path).map_err(write_failed(path))?;
    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o400);
    }
    #[cfg(windows)]
    {
        permissions.set_readonly(true);
    }
    fs::set_permissions(path, permissions).map_err(write_failed(path))
}

/// 目录 fsync 仅 Unix 可行（Windows 无法持目录句柄）。
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    {
        if let Ok(handle) = fs::File::open(dir) {
            let _ = handle.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = dir;
}

fn write_failed(path: impl AsRef<Path>) -> impl Fn(std::io::Error) -> AttachmentError {
    let path = path.as_ref().display().to_string();
    move |_| {
        AttachmentError::new("ATTACHMENT_WRITE_FAILED", format!("failed to write {path}"))
    }
}

fn corrupt(path: &Path) -> AttachmentError {
    AttachmentError::new(
        "ATTACHMENT_CORRUPT",
        format!("stored object digest mismatch at {}", path.display()),
    )
}

/// 读对象文件：缺失 → `ATTACHMENT_NOT_FOUND`，其余 IO 故障 → `ATTACHMENT_READ_FAILED`。
fn read_object(path: &Path) -> Result<Vec<u8>, AttachmentError> {
    match fs::read(path) {
        Ok(data) => Ok(data),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(AttachmentError::new(
            "ATTACHMENT_NOT_FOUND",
            format!("attachment object {} does not exist", path.display()),
        )),
        Err(_) => Err(AttachmentError::new(
            "ATTACHMENT_READ_FAILED",
            format!("failed to read attachment object {}", path.display()),
        )),
    }
}

/// 校验 `sha256:<64hex>` 形态并返回裸 hex。
fn parse_attachment_id(id: &str) -> Result<String, AttachmentError> {
    let hex = id
        .strip_prefix("sha256:")
        .ok_or_else(|| invalid_ref(id))?;
    if hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(hex.to_ascii_lowercase());
    }
    Err(invalid_ref(id))
}

fn invalid_ref(id: &str) -> AttachmentError {
    AttachmentError::new("INVALID_ATTACHMENT_REF", format!("malformed attachment id: {id}"))
}

fn verify_bytes(
    data: &[u8],
    reference: &FileAttachmentRef,
    hex: &str,
) -> Result<(), AttachmentError> {
    if sha256_hex(data) != hex || data.len() as u64 != reference.bytes {
        return Err(AttachmentError::new(
            "ATTACHMENT_CORRUPT",
            format!("stored object does not match {}", reference.attachment_id),
        ));
    }
    Ok(())
}

fn verify_image(
    data: &[u8],
    reference: &ImageAttachmentRef,
    hex: &str,
) -> Result<(), AttachmentError> {
    verify_bytes(data, &file_view(reference), hex)?;
    match detect_image(data) {
        Ok((media_type, width, height))
            if media_type == reference.media_type
                && width == reference.width
                && height == reference.height =>
        {
            Ok(())
        }
        _ => Err(AttachmentError::new(
            "ATTACHMENT_CORRUPT",
            format!("stored image metadata mismatch for {}", reference.attachment_id),
        )),
    }
}

/// 图片引用按文件视图做字节/摘要复核（字段同构子集）。
fn file_view(reference: &ImageAttachmentRef) -> FileAttachmentRef {
    FileAttachmentRef {
        attachment_id: reference.attachment_id.clone(),
        name: String::new(),
        bytes: reference.bytes,
    }
}

/// 媒体类型 + 内蕴尺寸嗅探：image crate guess-format（magic）+ 尺寸解码。
pub(crate) fn detect_image(data: &[u8]) -> Result<(String, u32, u32), AttachmentError> {
    if data.is_empty() {
        return Err(AttachmentError::new(
            "INVALID_IMAGE",
            "image must not be empty",
        ));
    }
    let reader = image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .map_err(|_| undetectable())?;
    let format = reader.format().ok_or_else(undetectable)?;
    let media_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        image::ImageFormat::Bmp => "image/bmp",
        _ => return Err(undetectable()),
    };
    let (width, height) = reader.into_dimensions().map_err(|_| undetectable())?;
    Ok((media_type.to_string(), width, height))
}

fn undetectable() -> AttachmentError {
    AttachmentError::new(
        "INVALID_IMAGE",
        "bytes are not a detectable image (png/jpeg/gif/webp/bmp)",
    )
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            hex.push_str(&format!("{byte:02x}"));
            hex
        })
}

/// 显示名净化：取叶名、删控制字符、去首尾空白、
/// 按 255 字符截断；净化后为空则丢弃（`None`）。
pub(crate) fn display_name(value: Option<&str>) -> Option<String> {
    let leaf = value?.rsplit(['/', '\\']).next().unwrap_or(value?);
    let cleaned: String = leaf.chars().filter(|c| !c.is_control()).collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return None;
    }
    // 按字符数截断（255 字符），非字节截断。
    Some(trimmed.chars().take(255).collect())
}

/// 文件叶名净化：叶名 → 删控制字符 → `<>:"|?*` 替换为
/// `_` → 去首尾空白与结尾点/空格 → Windows 保留设备名前缀 `_` → UTF-8 ≤255
/// 字节截断 → 再去结尾点/空格 → 空则回退 `file`。
pub(crate) fn file_leaf_name(value: Option<&str>) -> String {
    let leaf = value
        .and_then(|v| v.rsplit(['/', '\\']).next())
        .unwrap_or_default();
    let stripped: String = leaf.chars().filter(|c| !c.is_control()).collect();
    let sanitized: String = stripped
        .chars()
        .map(|c| if "<>:\"|?*".contains(c) { '_' } else { c })
        .collect();
    let mut clean = sanitized.trim().trim_end_matches(['.', ' ']).to_string();
    if reserved_windows_device(&clean) {
        clean = format!("_{clean}");
    }
    let capped = cap_utf8(&clean, 255);
    let capped = capped.trim_end_matches(['.', ' ']);
    if capped.is_empty() {
        "file".to_string()
    } else {
        capped.to_string()
    }
}

/// Windows 保留设备名（含扩展名形态 `CON.txt`），大小写不敏感；stem 匹配前
/// 剥结尾点/空格。
fn reserved_windows_device(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let stem = stem.trim_end_matches(['.', ' ']);
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6"
            | "COM7" | "COM8" | "COM9" | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6"
            | "LPT7" | "LPT8" | "LPT9"
    )
}

/// UTF-8 串按字节预算截断（不切多字节字符）。
fn cap_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
