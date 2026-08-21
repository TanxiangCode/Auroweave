#import <Cocoa/Cocoa.h>

/// 递归查找并设置所有 NSStatusBarButton 的 AttributedString
static BOOL apply_attributed_title_to_view(NSView *view, NSAttributedString *attrString) {
    if (!view) return NO;
    BOOL found = NO;
    if ([view isKindOfClass:[NSButton class]]) {
        NSButton *btn = (NSButton *)view;
        [btn setAttributedTitle:attrString];
        found = YES;
    }
    for (NSView *sub in view.subviews) {
        if (apply_attributed_title_to_view(sub, attrString)) {
            found = YES;
        }
    }
    return found;
}

/// 设置 macOS 状态栏托盘按钮的富文本标题（7.3pt 常规不加粗等宽数字，双排高度 16pt 与图标 1:1 绝对垂直居中对齐，右对齐）
void macos_set_tray_attributed_title(const char *text) {
    NSString *capturedString = nil;
    if (text && strlen(text) > 0) {
        capturedString = [NSString stringWithUTF8String:text];
    }
    if (!capturedString) {
        capturedString = @"";
    }

    dispatch_async(dispatch_get_main_queue(), ^{
        if ([capturedString length] == 0) {
            NSAttributedString *emptyAttr = [[NSAttributedString alloc] initWithString:@""];
            for (NSWindow *window in [NSApplication sharedApplication].windows) {
                if ([window isKindOfClass:NSClassFromString(@"NSStatusBarWindow")]) {
                    apply_attributed_title_to_view(window.contentView, emptyAttr);
                }
            }
            return;
        }

        NSMutableParagraphStyle *paragraphStyle = [[NSMutableParagraphStyle alloc] init];
        paragraphStyle.alignment = NSTextAlignmentRight;
        paragraphStyle.maximumLineHeight = 8.0;
        paragraphStyle.minimumLineHeight = 8.0;
        paragraphStyle.lineSpacing = 0.0;
        paragraphStyle.paragraphSpacing = 0.0;

        // 使用 7.3pt 常规字重 (Regular) 等宽数字字体，配合 -2.1pt 基线负偏移，使双排文字与左侧图标绝对垂直居中
        NSDictionary *attrs = @{
            NSFontAttributeName: [NSFont monospacedDigitSystemFontOfSize:7.3 weight:NSFontWeightRegular],
            NSParagraphStyleAttributeName: paragraphStyle,
            NSBaselineOffsetAttributeName: @(-2.1),
        };

        NSAttributedString *attrString = [[NSAttributedString alloc] initWithString:capturedString attributes:attrs];

        for (NSWindow *window in [NSApplication sharedApplication].windows) {
            if ([window isKindOfClass:NSClassFromString(@"NSStatusBarWindow")]) {
                apply_attributed_title_to_view(window.contentView, attrString);
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
