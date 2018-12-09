//
//  ViewItemHeader.swift
//  ced
//
//  Created by Martin Chaine on 28/10/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Cocoa

class ViewItemHeaderView: NSStackView {

    convenience init(buffer: String, range: (uint, uint)) {
        let fileView = NSTextField(string: buffer)
        fileView.isEditable = false
        fileView.isBordered = false
        let rangeView = NSTextField(string: "[\(range.0):\(range.1)]")
        rangeView.isEditable = false
        rangeView.isBordered = false
        self.init(views: [fileView, rangeView])
    }
    
}
