# threemu - Nintendo 3DS Emulator

## Usage

### Basic usage

```bash
just emu <path-to-firm-file>
```

### With SD Card Support

```bash
just emu <path-to-firm-file> --sd-card <path-to-sd-image>

# Optionally the FIRM can be loaded from the SD image
just emu <path-in-sd-card> --sd-card <path-to-sd-image> --entry-firm-in-sd-card
```

## Examples

Run [3DS Linux](https://github.com/linux-3ds) starting from the [firm_linux_loader](https://github.com/linux-3ds/firm_linux_loader):

```bash
just emu-linux ../linux-3ds/output/sdcard.img 
```

![threemu emulating Linux bootloader](https://github.com/user-attachments/assets/c1820851-b531-4190-ae02-77ecab223b3e)