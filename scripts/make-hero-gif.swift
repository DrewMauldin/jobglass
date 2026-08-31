#!/usr/bin/env swift

import Foundation
import ImageIO
import UniformTypeIdentifiers

private let arguments = CommandLine.arguments.dropFirst()

guard arguments.count >= 2 else {
    FileHandle.standardError.write(
        Data("usage: make-hero-gif.swift FRAME... OUTPUT\n".utf8)
    )
    exit(64)
}

let paths = Array(arguments)
let outputPath = paths[paths.count - 1]
let framePaths = paths.dropLast()
let outputURL = URL(fileURLWithPath: outputPath)

guard
    let destination = CGImageDestinationCreateWithURL(
        outputURL as CFURL,
        UTType.gif.identifier as CFString,
        framePaths.count,
        nil
    )
else {
    FileHandle.standardError.write(Data("could not create GIF destination\n".utf8))
    exit(1)
}

let loopProperties = [
    kCGImagePropertyGIFDictionary: [
        kCGImagePropertyGIFLoopCount: 0,
    ],
] as CFDictionary
CGImageDestinationSetProperties(destination, loopProperties)

for path in framePaths {
    let url = URL(fileURLWithPath: path)
    guard
        let source = CGImageSourceCreateWithURL(url as CFURL, nil),
        let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else {
        FileHandle.standardError.write(Data("could not read frame: \(path)\n".utf8))
        exit(1)
    }

    let frameProperties = [
        kCGImagePropertyGIFDictionary: [
            kCGImagePropertyGIFDelayTime: 1.8,
            kCGImagePropertyGIFUnclampedDelayTime: 1.8,
        ],
    ] as CFDictionary
    CGImageDestinationAddImage(destination, image, frameProperties)
}

guard CGImageDestinationFinalize(destination) else {
    FileHandle.standardError.write(Data("could not finalise GIF\n".utf8))
    exit(1)
}
