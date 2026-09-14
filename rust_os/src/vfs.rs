extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Clone)]
pub struct Inode {
    pub name: String,
    pub is_dir: bool,
    pub data: Vec<u8>,
    pub children: Vec<Inode>,
    pub perms: String,
    pub owner: String,
    pub group: String,
    pub date: String,
}

pub static VFS_ROOT: Mutex<Option<Inode>> = Mutex::new(None);

static INITFS_DATA: &[u8] = include_bytes!("initfs.tar");

pub fn init_vfs() {
    let root = Inode {
        name: String::from(""),
        is_dir: true,
        data: Vec::new(),
        children: Vec::new(),
        perms: String::from("drwxr-xr-x"),
        owner: String::from("root"),
        group: String::from("root"),
        date: String::from("Jul 20 00:00"),
    };

    *VFS_ROOT.lock() = Some(root);

    if let Err(e) = parse_tar(INITFS_DATA) {
        crate::println!("VFS: Failed to parse initfs.tar: {}", e);
    }
}

pub fn parse_tar(data: &[u8]) -> Result<(), &'static str> {
    let mut offset = 0;
    while offset + 512 <= data.len() {
        let header = &data[offset..offset + 512];
        if header.iter().all(|&b| b == 0) {
            break;
        }

        let mut name_len = 0;
        while name_len < 100 && header[name_len] != 0 {
            name_len += 1;
        }
        if name_len == 0 {
            offset += 512;
            continue;
        }
        let name_str = core::str::from_utf8(&header[..name_len]).map_err(|_| "Invalid UTF-8 filename")?;

        let size_str = core::str::from_utf8(&header[124..136]).map_err(|_| "Invalid UTF-8 size")?;
        let mut size = 0;
        for c in size_str.trim().chars() {
            if c >= '0' && c <= '7' {
                size = size * 8 + (c as usize - '0' as usize);
            } else {
                break;
            }
        }

        let type_flag = header[156];
        let file_path = alloc::format!("/{}", name_str.trim_end_matches('/'));

        if type_flag == b'5' {
            if file_path != "/" {
                let _ = vfs_mkdir(&file_path, "root", "root", "drwxr-xr-x");
            }
        } else {
            let data_start = offset + 512;
            if data_start + size <= data.len() {
                let file_data = &data[data_start..data_start + size];
                let owner = if file_path.contains("/home/guru") { "guru" } else { "root" };
                let group = if file_path.contains("/home/guru") { "guru" } else { "root" };
                let perms = if file_path.contains("/var/log") { "-rw-r-----" } else if file_path.contains("/bin") { "-rwxr-xr-x" } else { "-rw-r--r--" };
                vfs_write_file(&file_path, file_data, owner, group, perms)?;
            }
        }

        let aligned_size = (size + 511) & !511;
        offset += 512 + aligned_size;
    }
    Ok(())
}

pub fn normalize_path(cwd: &str, path: &str) -> String {
    let absolute = if path.starts_with('/') {
        String::from(path)
    } else {
        let mut base = String::from(cwd);
        if !base.ends_with('/') {
            base.push('/');
        }
        base.push_str(path);
        base
    };

    let mut segments = Vec::new();
    for segment in absolute.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            segments.pop();
        } else {
            segments.push(segment);
        }
    }

    let mut result = String::from("/");
    for (i, seg) in segments.iter().enumerate() {
        if i > 0 {
            result.push('/');
        }
        result.push_str(seg);
    }
    result
}

fn find_node_mut<'a>(start_node: &'a mut Inode, path: &str) -> Option<&'a mut Inode> {
    let clean_path = path.trim_start_matches('/');
    if clean_path.is_empty() {
        return Some(start_node);
    }

    let mut current = start_node;
    for segment in clean_path.split('/') {
        if segment.is_empty() {
            continue;
        }
        let mut found_idx = None;
        for (i, child) in current.children.iter().enumerate() {
            if child.name == segment {
                found_idx = Some(i);
                break;
            }
        }
        if let Some(idx) = found_idx {
            current = &mut current.children[idx];
        } else {
            return None;
        }
    }
    Some(current)
}

fn vfs_write_file_internal(
    root: &mut Inode,
    path: &str,
    data: &[u8],
    owner: &str,
    group: &str,
    perms: &str,
) -> Result<(), &'static str> {
    let normalized = normalize_path("/", path);
    
    // Find the directory containing the file
    let last_slash = normalized.rfind('/').ok_or("Invalid path")?;
    let (parent_path, file_name) = if last_slash == 0 {
        ("/", &normalized[1..])
    } else {
        (&normalized[..last_slash], &normalized[last_slash+1..])
    };

    let parent = find_node_mut(root, parent_path).ok_or("Parent directory not found")?;
    if !parent.is_dir {
        return Err("Parent is not a directory");
    }

    let mut found_idx = None;
    for (i, child) in parent.children.iter().enumerate() {
        if child.name == file_name {
            found_idx = Some(i);
            break;
        }
    }

    if let Some(idx) = found_idx {
        if parent.children[idx].is_dir {
            return Err("Cannot overwrite directory with file");
        }
        parent.children[idx].data = Vec::from(data);
        parent.children[idx].date = String::from("Jul 20 00:00");
    } else {
        let new_file = Inode {
            name: String::from(file_name),
            is_dir: false,
            data: Vec::from(data),
            children: Vec::new(),
            perms: String::from(perms),
            owner: String::from(owner),
            group: String::from(group),
            date: String::from("Jul 20 00:00"),
        };
        parent.children.push(new_file);
    }
    Ok(())
}

pub fn vfs_write_file(
    path: &str,
    data: &[u8],
    owner: &str,
    group: &str,
    perms: &str,
) -> Result<(), &'static str> {
    let mut lock = VFS_ROOT.lock();
    let root = lock.as_mut().ok_or("VFS not initialized")?;
    vfs_write_file_internal(root, path, data, owner, group, perms)
}

pub fn vfs_read_file(path: &str) -> Result<Vec<u8>, &'static str> {
    let mut lock = VFS_ROOT.lock();
    let root = lock.as_mut().ok_or("VFS not initialized")?;
    let node = find_node_mut(root, path).ok_or("File not found")?;
    if node.is_dir {
        return Err("Cannot read directory as file");
    }
    Ok(node.data.clone())
}

pub fn vfs_mkdir(
    path: &str,
    owner: &str,
    group: &str,
    perms: &str,
) -> Result<(), &'static str> {
    let mut lock = VFS_ROOT.lock();
    let root = lock.as_mut().ok_or("VFS not initialized")?;
    let normalized = normalize_path("/", path);
    let last_slash = normalized.rfind('/').ok_or("Invalid path")?;
    let (parent_path, dir_name) = if last_slash == 0 {
        ("/", &normalized[1..])
    } else {
        (&normalized[..last_slash], &normalized[last_slash+1..])
    };

    let parent = find_node_mut(root, parent_path).ok_or("Parent directory not found")?;
    if !parent.is_dir {
        return Err("Parent is not a directory");
    }

    for child in &parent.children {
        if child.name == dir_name {
            return Err("File or directory already exists");
        }
    }

    let new_dir = Inode {
        name: String::from(dir_name),
        is_dir: true,
        data: Vec::new(),
        children: Vec::new(),
        perms: String::from(perms),
        owner: String::from(owner),
        group: String::from(group),
        date: String::from("Jul 20 00:00"),
    };
    parent.children.push(new_dir);
    Ok(())
}

pub fn vfs_readdir(path: &str) -> Result<Vec<Inode>, &'static str> {
    let mut lock = VFS_ROOT.lock();
    let root = lock.as_mut().ok_or("VFS not initialized")?;
    let node = find_node_mut(root, path).ok_or("Directory not found")?;
    if !node.is_dir {
        return Err("Not a directory");
    }
    Ok(node.children.clone())
}

pub fn vfs_remove(path: &str) -> Result<(), &'static str> {
    let mut lock = VFS_ROOT.lock();
    let root = lock.as_mut().ok_or("VFS not initialized")?;
    let normalized = normalize_path("/", path);
    let last_slash = normalized.rfind('/').ok_or("Invalid path")?;
    let (parent_path, name) = if last_slash == 0 {
        ("/", &normalized[1..])
    } else {
        (&normalized[..last_slash], &normalized[last_slash+1..])
    };

    let parent = find_node_mut(root, parent_path).ok_or("Parent directory not found")?;
    let mut found_idx = None;
    for (i, child) in parent.children.iter().enumerate() {
        if child.name == name {
            found_idx = Some(i);
            break;
        }
    }

    if let Some(idx) = found_idx {
        parent.children.remove(idx);
        Ok(())
    } else {
        Err("File or directory not found")
    }
}

pub fn vfs_copy_unpacked(pkg_name: &str) -> Result<(), &'static str> {
    let staging_base = alloc::format!("/usr/share/apt/unpacked/{}", pkg_name);
    let mut file_paths = Vec::new();

    {
        let mut lock = VFS_ROOT.lock();
        let root = lock.as_mut().ok_or("VFS not initialized")?;
        if let Some(staging_node) = find_node_mut(root, &staging_base) {
            collect_files_recursive(staging_node, &staging_base, &mut file_paths);
        } else {
            return Err("Staging directory not found");
        }
    }

    for (rel_path, data) in file_paths {
        let owner = if rel_path.contains("/home/guru") { "guru" } else { "root" };
        let group = if rel_path.contains("/home/guru") { "guru" } else { "root" };
        
        // Ensure parent directories exist
        let normalized = normalize_path("/", &rel_path);
        let last_slash = normalized.rfind('/').ok_or("Invalid path")?;
        let parent_path = if last_slash == 0 { "/" } else { &normalized[..last_slash] };
        
        let _ = vfs_mkdir(parent_path, owner, group, "drwxr-xr-x");
        
        let perms = if rel_path.contains("/bin") { "-rwxr-xr-x" } else { "-rw-r--r--" };
        vfs_write_file(&rel_path, &data, owner, group, perms)?;
    }

    Ok(())
}

fn collect_files_recursive(node: &Inode, current_path: &str, files: &mut Vec<(String, Vec<u8>)>) {
    if node.is_dir {
        for child in &node.children {
            let next_path = if current_path == "/" {
                alloc::format!("/{}", child.name)
            } else {
                alloc::format!("{}/{}", current_path, child.name)
            };
            collect_files_recursive(child, &next_path, files);
        }
    } else {
        let parts: Vec<&str> = current_path.split('/').collect();
        if parts.len() > 6 {
            // parts[0] = "", parts[1] = "usr", parts[2] = "share", parts[3] = "apt", parts[4] = "unpacked", parts[5] = pkg_name
            let dest_parts = &parts[6..];
            let dest_path = alloc::format!("/{}", dest_parts.join("/"));
            files.push((dest_path, node.data.clone()));
        }
    }
}

pub fn parse_tar_to_staging(data: &[u8], pkg_name: &str) -> Result<(), &'static str> {
    let mut offset = 0;
    while offset + 512 <= data.len() {
        let header = &data[offset..offset + 512];
        if header.iter().all(|&b| b == 0) {
            break;
        }

        let mut name_len = 0;
        while name_len < 100 && header[name_len] != 0 {
            name_len += 1;
        }
        if name_len == 0 {
            offset += 512;
            continue;
        }
        let name_str = core::str::from_utf8(&header[..name_len]).map_err(|_| "Invalid UTF-8 filename")?;

        let size_str = core::str::from_utf8(&header[124..136]).map_err(|_| "Invalid UTF-8 size")?;
        let mut size = 0;
        for c in size_str.trim().chars() {
            if c >= '0' && c <= '7' {
                size = size * 8 + (c as usize - '0' as usize);
            } else {
                break;
            }
        }

        let type_flag = header[156];
        let file_path = alloc::format!("/usr/share/apt/unpacked/{}/{}", pkg_name, name_str.trim_end_matches('/'));

        if type_flag == b'5' {
            let _ = vfs_mkdir(&file_path, "root", "root", "drwxr-xr-x");
        } else {
            let data_start = offset + 512;
            if data_start + size <= data.len() {
                let file_data = &data[data_start..data_start + size];
                let owner = if file_path.contains("/home/guru") { "guru" } else { "root" };
                let group = if file_path.contains("/home/guru") { "guru" } else { "root" };
                
                let normalized = normalize_path("/", &file_path);
                if let Some(last_slash) = normalized.rfind('/') {
                    let parent = if last_slash == 0 { "/" } else { &normalized[..last_slash] };
                    let _ = vfs_mkdir(parent, owner, group, "drwxr-xr-x");
                }

                let perms = if file_path.contains("/bin") { "-rwxr-xr-x" } else { "-rw-r--r--" };
                vfs_write_file(&file_path, file_data, owner, group, perms)?;
            }
        }

        let aligned_size = (size + 511) & !511;
        offset += 512 + aligned_size;
    }
    Ok(())
}
