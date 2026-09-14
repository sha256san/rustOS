#!/usr/bin/env python3
import sys
import os
import struct

def create_fat12_efi(bootx64_path, output_img_path):
    """
    Creates a standard 1.44MB FAT12 EFI system partition disk image containing /EFI/BOOT/BOOTX64.EFI.
    """
    if not os.path.exists(bootx64_path):
        print(f"Error: {bootx64_path} not found.", file=sys.stderr)
        sys.exit(1)

    with open(bootx64_path, 'rb') as f:
        file_data = f.read()
    file_size = len(file_data)

    total_sectors = 2880
    bytes_per_sector = 512
    img_size = total_sectors * bytes_per_sector
    img = bytearray(img_size)

    # 1. Boot sector
    boot_sec = bytearray(512)
    boot_sec[0:3] = b'\xeb\x3c\x90'
    boot_sec[3:11] = b'MSWIN4.1'
    struct.pack_into('<H', boot_sec, 0x0B, 512) # bytes per sector
    boot_sec[0x0D] = 1 # sectors per cluster
    struct.pack_into('<H', boot_sec, 0x0E, 1)   # reserved sectors
    boot_sec[0x10] = 2 # num fats
    struct.pack_into('<H', boot_sec, 0x11, 224) # root entries
    struct.pack_into('<H', boot_sec, 0x13, 2880)# total sectors
    boot_sec[0x15] = 0xF0 # media descriptor
    struct.pack_into('<H', boot_sec, 0x16, 9)   # sectors per FAT
    struct.pack_into('<H', boot_sec, 0x18, 18)  # sectors per track
    struct.pack_into('<H', boot_sec, 0x1A, 2)   # heads
    boot_sec[0x26] = 0x29
    struct.pack_into('<I', boot_sec, 0x27, 0x12345678)
    boot_sec[0x2B:0x36] = b'EFI SYSTEM '
    boot_sec[0x36:0x3E] = b'FAT12   '
    boot_sec[510:512] = b'\x55\xaa'
    img[0:512] = boot_sec

    # Offsets
    fat1_offset = 1 * 512
    fat2_offset = (1 + 9) * 512
    root_offset = (1 + 9 + 9) * 512
    root_size = 224 * 32 # 7168 bytes
    data_offset = root_offset + root_size

    # Media header
    img[fat1_offset:fat1_offset+3] = b'\xf0\xff\xff'
    img[fat2_offset:fat2_offset+3] = b'\xf0\xff\xff'

    def set_fat12_entry(cluster, value):
        fat_idx = fat1_offset + (cluster * 3) // 2
        if cluster % 2 == 0:
            val_existing = struct.unpack('<H', img[fat_idx:fat_idx+2])[0]
            val_new = (val_existing & 0xF000) | (value & 0x0FFF)
            struct.pack_into('<H', img, fat_idx, val_new)
        else:
            val_existing = struct.unpack('<H', img[fat_idx:fat_idx+2])[0]
            val_new = (val_existing & 0x000F) | ((value & 0x0FFF) << 4)
            struct.pack_into('<H', img, fat_idx, val_new)
        
        img[fat2_offset + (fat_idx - fat1_offset)] = img[fat_idx]
        img[fat2_offset + (fat_idx - fat1_offset) + 1] = img[fat_idx + 1]

    # Cluster 2 = EFI dir, Cluster 3 = BOOT dir
    set_fat12_entry(2, 0xFFF)
    set_fat12_entry(3, 0xFFF)

    # Root entry for EFI
    root_entry = bytearray(32)
    root_entry[0:11] = b'EFI        '
    root_entry[11] = 0x10
    struct.pack_into('<H', root_entry, 26, 2)
    img[root_offset:root_offset+32] = root_entry

    # EFI dir entry for BOOT
    cluster2_offset = data_offset + (2 - 2) * 512
    boot_entry = bytearray(32)
    boot_entry[0:11] = b'BOOT       '
    boot_entry[11] = 0x10
    struct.pack_into('<H', boot_entry, 26, 3)
    img[cluster2_offset:cluster2_offset+32] = boot_entry

    # BOOT dir entry for BOOTX64.EFI
    cluster3_offset = data_offset + (3 - 2) * 512
    num_clusters = (file_size + 511) // 512
    start_cluster = 4

    for i in range(num_clusters):
        curr_cl = start_cluster + i
        next_cl = 0xFFF if i == num_clusters - 1 else curr_cl + 1
        set_fat12_entry(curr_cl, next_cl)

    file_entry = bytearray(32)
    file_entry[0:11] = b'BOOTX64 EFI'
    file_entry[11] = 0x20
    struct.pack_into('<H', file_entry, 26, start_cluster)
    struct.pack_into('<I', file_entry, 28, file_size)
    img[cluster3_offset:cluster3_offset+32] = file_entry

    # Write file data
    file_data_offset = data_offset + (start_cluster - 2) * 512
    img[file_data_offset:file_data_offset+file_size] = file_data

    os.makedirs(os.path.dirname(os.path.abspath(output_img_path)), exist_ok=True)
    with open(output_img_path, 'wb') as f:
        f.write(img)

if __name__ == '__main__':
    if len(sys.argv) < 3:
        print("Usage: make_efi_img.py <path_to_BOOTX64.EFI> <output_efi.img>")
        sys.exit(1)
    create_fat12_efi(sys.argv[1], sys.argv[2])
