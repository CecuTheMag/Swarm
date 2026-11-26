import PDFKit
import AppKit
// usage: pdf.swift page1.png page2.png out.pdf  – each image becomes one A4 landscape page
let a = CommandLine.arguments
let doc = PDFDocument()
let pageSize = NSSize(width: 841.89, height: 595.28)   // A4 landscape in points
for (i, path) in a[1..<(a.count - 1)].enumerated() {
  guard let img = NSImage(contentsOfFile: path) else { fatalError("cannot read \(path)") }
  img.size = pageSize
  doc.insert(PDFPage(image: img)!, at: i)
}
doc.write(toFile: a[a.count - 1])
