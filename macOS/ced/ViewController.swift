//
//  ViewController.swift
//  ced
//
//  Created by Martin Chaine on 18/06/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Cocoa

class ViewController: NSViewController {

    @IBOutlet var buffer: NSTextView!
    
    var ced: Ced!
    var session: String!
    
    override func viewDidLoad() {
        super.viewDidLoad()

        let handler = ConnectionController(buffer: self.buffer)
        self.ced = Ced(handler: handler)
        
        self.ced.connect(session: self.session)
    }
    
    override func viewDidAppear() {
        self.ced.handler.window = self.view.window
    }

    override var representedObject: Any? {
        didSet {
        // Update the view, if already loaded.
        }
    }
    
    deinit {
        self.ced.close()
    }
    
}
