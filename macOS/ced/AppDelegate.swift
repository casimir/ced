//
//  AppDelegate.swift
//  ced
//
//  Created by Martin Chaine on 18/06/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Cocoa

@NSApplicationMain
class AppDelegate: NSObject, NSApplicationDelegate {
    
    var controllers: [ViewController] = []

    func applicationDidFinishLaunching(_ aNotification: Notification) {
        // Insert code here to initialize your application
    }

    func applicationWillTerminate(_ aNotification: Notification) {
        // Insert code here to tear down your application
    }
    
    func focusedController() -> ViewController {
        return NSApplication.shared.mainWindow!.contentViewController as! ViewController
    }

    func newWindow(session: String? = nil) {
        let storyboard = NSStoryboard(name: "Main", bundle: nil)
        let identifier = "bufferController"
        let controller = storyboard.instantiateController(withIdentifier: identifier) as! ViewController
        controller.session = session
        self.controllers.append(controller)
        let window = NSWindow(contentViewController: controller)
        window.makeKeyAndOrderFront(self)
        let windowController = NSWindowController(window: window)
        windowController.showWindow(self)
    }
    
    @IBAction func newWindow(sender: AnyObject) {
        let session = self.focusedController().ced.context.session
        self.newWindow(session: session)
    }
    
    @objc func connectNewSession(sender: NSMenuItem) {
        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 200, height: 24))
        let dialog = NSAlert()
        dialog.messageText = "New session name"
        dialog.accessoryView = input
        dialog.window.initialFirstResponder = input
        dialog.addButton(withTitle: "Ok")
        dialog.addButton(withTitle: "Cancel")
        if dialog.runModal() == .alertFirstButtonReturn {
            if input.stringValue.count > 0 {
                self.newWindow(session: input.stringValue)
            }
        }
    }
    
    @objc func connectSession(sender: NSMenuItem) {
        self.newWindow(session: sender.title)
    }
    
    @IBAction func openFile(sender: NSMenuItem) {
        let dialog = NSOpenPanel()
        dialog.canChooseDirectories = false
        dialog.canChooseFiles = true
        dialog.canCreateDirectories = false
        dialog.allowsMultipleSelection = true
        if dialog.runModal() == .OK {
            let paths = dialog.urls.map { $0.path }
            let ced = self.focusedController().ced!
            ced.request(method: "edit", params: paths)
        }
    }
    
    @objc func selectBuffer(sender: NSMenuItem) {
        let ced = self.focusedController().ced!
        ced.request(method: "buffer-select", params: [sender.title])
    }

}

