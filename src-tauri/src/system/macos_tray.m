#import <Cocoa/Cocoa.h>

/// 判断视图是否为 Auroweave 自己的 Tauri 状态栏按钮。
/// Tauri 会在 NSStatusBarButton 下挂载 TaoTrayTarget 点击层，以它作为可靠边界，
/// 避免把同进程中的其他状态栏项目一起改写。
static BOOL is_auroweave_tray_button(NSView *view) {
    if (![view isKindOfClass:[NSButton class]]) return NO;

    Class trayTargetClass = NSClassFromString(@"TaoTrayTarget");
    if (!trayTargetClass) return NO;
    for (NSView *subview in ((NSButton *)view).subviews) {
        if ([subview isKindOfClass:trayTargetClass]) return YES;
    }
    return NO;
}

/// 在状态栏窗口树中只查找 Auroweave 自己的按钮。
static NSButton *find_auroweave_tray_button(NSView *view) {
    if (is_auroweave_tray_button(view)) {
        return (NSButton *)view;
    }
    for (NSView *subview in view.subviews) {
        NSButton *button = find_auroweave_tray_button(subview);
        if (button) return button;
    }
    return nil;
}

/// 设置 macOS 状态栏网速为双排定宽布局。
///
/// 上、下行各自使用固定宽度槽位，并以 7.5pt 系统等宽字体和 8pt 固定行高呈现。
/// 总高度 16pt，可与 18pt 托盘图标一起稳定地垂直居中，不依赖负基线偏移。
void macos_set_tray_attributed_title(const char *text) {
    // dispatch 会复制 Block 并保留 Objective-C 对象；使用 alloc/init 避免
    // 跨线程调用时把自动释放对象交给无 pool 的 Rust 工作线程。
    NSString *title = text && strlen(text) > 0
        ? [[NSString alloc] initWithUTF8String:text]
        : @"";

    dispatch_async(dispatch_get_main_queue(), ^{
        @autoreleasepool {
            NSAttributedString *attributedTitle = nil;
            if ([title length] > 0) {
                NSMutableParagraphStyle *paragraphStyle =
                    [[[NSMutableParagraphStyle alloc] init] autorelease];
                paragraphStyle.alignment = NSTextAlignmentLeft;
                paragraphStyle.minimumLineHeight = 8.0;
                paragraphStyle.maximumLineHeight = 8.0;
                paragraphStyle.lineBreakMode = NSLineBreakByClipping;
                paragraphStyle.lineSpacing = 0.0;
                paragraphStyle.paragraphSpacing = 0.0;
                paragraphStyle.hyphenationFactor = 0.0;

                NSDictionary *attributes = @{
                    NSFontAttributeName: [NSFont monospacedSystemFontOfSize:7.5
                                                                    weight:NSFontWeightRegular],
                    NSParagraphStyleAttributeName: paragraphStyle,
                    NSKernAttributeName: @0.0,
                };
                attributedTitle = [[[NSAttributedString alloc] initWithString:title
                                                                   attributes:attributes] autorelease];
            }

            for (NSWindow *window in [NSApplication sharedApplication].windows) {
                if (![window isKindOfClass:NSClassFromString(@"NSStatusBarWindow")]) continue;
                NSButton *button = find_auroweave_tray_button(window.contentView);
                if (!button) continue;

                // Tauri 在 title=None 时不会主动清空 NSButton，这里显式清空普通标题，
                // 防止旧文本在富文本样式变化或系统回退时重新出现。
                [button setTitle:@""];
                [button setAttributedTitle:attributedTitle];
            }
        }
    });
}

/// 提取指定可执行文件或 .app bundle 的原生 48x48 PNG Base64 图标
char* macos_get_app_icon_base64(const char* exe_path) {
    if (!exe_path || strlen(exe_path) == 0) return NULL;
    @autoreleasepool {
        NSString *path = [NSString stringWithUTF8String:exe_path];
        if (!path) return NULL;

        // 尝试定位 .app 根目录（如 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome -> /Applications/Google Chrome.app）
        NSRange appRange = [path rangeOfString:@".app" options:NSCaseInsensitiveSearch | NSBackwardsSearch];
        NSString *targetPath = path;
        if (appRange.location != NSNotFound) {
            targetPath = [path substringToIndex:appRange.location + appRange.length];
        }

        if (![[NSFileManager defaultManager] fileExistsAtPath:targetPath]) {
            return NULL;
        }

        NSImage *icon = [[NSWorkspace sharedWorkspace] iconForFile:targetPath];
        if (!icon) return NULL;

        NSBitmapImageRep *rep = [[NSBitmapImageRep alloc]
            initWithBitmapDataPlanes:NULL
                          pixelsWide:48
                          pixelsHigh:48
                       bitsPerSample:8
                     samplesPerPixel:4
                            hasAlpha:YES
                            isPlanar:NO
                      colorSpaceName:NSDeviceRGBColorSpace
                         bytesPerRow:48 * 4
                        bitsPerPixel:32];

        if (!rep) return NULL;

        [NSGraphicsContext saveGraphicsState];
        [NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithBitmapImageRep:rep]];
        [icon drawInRect:NSMakeRect(0, 0, 48, 48)
                fromRect:NSZeroRect
               operation:NSCompositingOperationCopy
                fraction:1.0];
        [NSGraphicsContext restoreGraphicsState];

        NSData *pngData = [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}];

        if (!pngData || [pngData length] == 0) return NULL;

        NSString *base64 = [pngData base64EncodedStringWithOptions:0];
        NSString *dataUri = [NSString stringWithFormat:@"data:image/png;base64,%@", base64];

        const char *utf8 = [dataUri UTF8String];
        if (!utf8) return NULL;

        return strdup(utf8);
    }
}

/// 释放由 C 分配的字符串内存
void macos_free_string(char *ptr) {
    if (ptr) {
        free(ptr);
    }
}
