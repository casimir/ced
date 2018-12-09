//
//  ViewItemLinesView.swift
//  ced
//
//  Created by Martin Chaine on 28/10/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Cocoa

class ViewItemLinesView: NSStackView {

    convenience init(lines: [String]) {
        self.init()
        self.translatesAutoresizingMaskIntoConstraints = false
        self.orientation = .vertical
        self.alignment = .bottom
        for line in lines {
            let view = NSTextField(string: line)
            view.isEditable = false
            view.isBordered = false
            self.addSubview(view)
        }
        for line in lines {
            let view = NSTextField(string: line)
            view.isEditable = false
            view.isBordered = false
            self.addSubview(view)
        }
        for line in lines {
            let view = NSTextField(string: line)
            view.isEditable = false
            view.isBordered = false
            self.addSubview(view)
        }
    }
    
}
