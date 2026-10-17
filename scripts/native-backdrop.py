import tkinter as tk
root=tk.Tk();root.title('EconomyWarRoom native test backdrop')
w=root.winfo_screenwidth();h=root.winfo_screenheight()
root.geometry(f'{w}x{h-80}+0+40')
canvas=tk.Canvas(root,highlightthickness=0);canvas.pack(fill='both',expand=True)
colors=['#18374a','#53c4cd','#73538a','#e4b477','#e8eee7']
for x in range(0,w,60):canvas.create_rectangle(x,0,x+60,h,fill=colors[(x//60)%len(colors)],outline='')
canvas.create_text(w//2,70,text='EconomyWarRoom · native backdrop test',font=('Segoe UI',28),fill='#10121a')
root.mainloop()
