use crate::{error::AppError, week::week_range};
use std::{fs::OpenOptions, io::Write, path::Path};

const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n";

pub fn save_week_png(
    directory: &str,
    start_date: &str,
    end_date: &str,
    png_bytes: &[u8],
) -> Result<String, AppError> {
    week_range(start_date)?;
    week_range(end_date)?;
    if !png_bytes.starts_with(PNG_HEADER) || png_bytes.len() > 20_000_000 {
        return Err(AppError::validation("导出图片无效"));
    }
    let folder = Path::new(directory.trim());
    if !folder.is_absolute() || !folder.is_dir() {
        return Err(AppError::validation("导出文件夹不存在，请检查地址"));
    }

    for index in 0..1000 {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!("_{index}")
        };
        let path = folder.join(format!("Time_{start_date}_{end_date}{suffix}.png"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if file.write_all(png_bytes).is_err() {
                    let _ = std::fs::remove_file(&path);
                    return Err(AppError::validation("图片保存失败，请检查文件夹权限"));
                }
                return Ok(path.to_string_lossy().into_owned());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(AppError::validation("图片保存失败，请检查文件夹权限")),
        }
    }
    Err(AppError::validation("同名导出图片过多，请更换文件夹"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_keeps_existing_images_and_rejects_missing_folder() {
        let folder = std::env::temp_dir().join(format!("time-export-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let png = PNG_HEADER;
        let first =
            save_week_png(folder.to_str().unwrap(), "2026-09-21", "2026-09-27", png).unwrap();
        let second =
            save_week_png(folder.to_str().unwrap(), "2026-09-21", "2026-09-27", png).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), png);
        assert!(save_week_png("", "2026-09-21", "2026-09-27", png).is_err());
        assert!(save_week_png(folder.to_str().unwrap(), "../bad", "2026-09-27", png).is_err());
        std::fs::remove_file(first).unwrap();
        std::fs::remove_file(second).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
