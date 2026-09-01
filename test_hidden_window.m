#import <Cocoa/Cocoa.h>

int main() {
    @autoreleasepool {
        NSWindowStyleMask mask = NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable;
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 800, 600) styleMask:mask backing:NSBackingStoreBuffered defer:YES];
        
        NSRect frame = [window frame];
        NSRect contentLayout = [window contentLayoutRect];
        
        printf("Frame height: %f\n", frame.size.height);
        printf("Content layout height: %f\n", contentLayout.size.height);
        printf("Diff: %f\n", frame.size.height - contentLayout.size.height);
        
        NSRect contentFrame = [[window contentView] frame];
        printf("Content frame height: %f\n", contentFrame.size.height);
        printf("Content diff: %f\n", frame.size.height - contentFrame.size.height);
    }
    return 0;
}
