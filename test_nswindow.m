#import <Cocoa/Cocoa.h>

int main() {
    @autoreleasepool {
        NSWindowStyleMask mask = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable;
        NSRect content = NSMakeRect(0, 0, 100, 100);
        NSRect frame = [NSWindow frameRectForContentRect:content styleMask:mask];
        printf("Standard Titlebar offset: %f\n", frame.size.height - content.size.height);
        
        NSWindowStyleMask fullSizeMask = mask | NSWindowStyleMaskFullSizeContentView;
        NSRect fullFrame = [NSWindow frameRectForContentRect:content styleMask:fullSizeMask];
        printf("FullSize Titlebar offset: %f\n", fullFrame.size.height - content.size.height);
    }
    return 0;
}
