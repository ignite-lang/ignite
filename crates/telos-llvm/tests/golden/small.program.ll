; ModuleID = 'tests.small'
source_filename = "tests.small"

@telos.empty = private constant [8 x i8] zeroinitializer, align 8
@telos.msg.index = private unnamed_addr constant [18 x i8] c"index out of range", align 1
@telos.msg.slice = private unnamed_addr constant [18 x i8] c"slice out of range", align 1
@telos.msg.divide = private unnamed_addr constant [25 x i8] c"attempt to divide by zero", align 1
@telos.msg.match = private unnamed_addr constant [12 x i8] c"no match arm", align 1
@.str.1 = private unnamed_addr constant [1 x i8] c"!", align 1
@.str.0 = private unnamed_addr constant [1 x i8] c"=", align 1
@.str.3 = private unnamed_addr constant [4 x i8] c"area", align 1
@.str.2 = private unnamed_addr constant [8 x i8] c"negative", align 1

declare void @telos_rt_init(i32, ptr)
declare ptr @telos_rt_alloc(i64, i64)
declare void @telos_rt_spawn(ptr, ptr)
declare void @telos_rt_spawn_ready()
declare void @telos_rt_task_end()
declare i64 @telos_rt_arena_mark()
declare void @telos_rt_arena_release(i64)
declare i8 @telos_rt_arena_due(i64, i64)
declare void @telos_rt_arena_scratch(i8)
declare void @telos_rt_arena_scratch_clear()
declare ptr @telos_rt_arena_state() nounwind willreturn speculatable memory(none)
declare i8 @telos_rt_spawn_copy(ptr, ptr, i64)
declare i32 @telos_rt_args_port(ptr, ptr, ptr)
declare void @telos_rt_exit(i32) noreturn
declare void @telos_rt_fail(ptr) noreturn
declare void @telos_rt_panic(ptr) noreturn
declare void @telos_rt_log(ptr)
declare i32 @telos_rt_tcp_bind(ptr, i64, ptr, ptr)
declare i32 @telos_rt_tcp_accept(i64, ptr, ptr)
declare i64 @telos_rt_tcp_local_port(i64)
declare i32 @telos_rt_tcp_connect(ptr, i64, ptr, ptr)
declare i32 @telos_rt_tcp_read(i64, i64, ptr, ptr)
declare i32 @telos_rt_tcp_write_all(i64, ptr, ptr)
declare i32 @telos_rt_tcp_close(i64, ptr)
declare void @telos_rt_tcp_release(i64)
declare i32 @telos_rt_stdin_read(i64, ptr, ptr)
declare i32 @telos_rt_stdin_is_terminal(ptr, ptr)
declare i32 @telos_rt_stdin_set_raw(i8, ptr)
declare i32 @telos_rt_stdout_write_all(ptr, ptr)
declare void @telos_rt_io_error_new(ptr, ptr)
declare void @telos_rt_text_from_bytes_lossy(ptr, ptr)
declare void @telos_rt_text_trim(ptr, ptr)
declare void @telos_rt_text_concat(ptr, ptr, ptr)
declare i8 @telos_rt_text_eq(ptr, ptr)
declare i8 @telos_rt_text_starts_with(ptr, ptr)
declare void @telos_rt_text_join(ptr, ptr, ptr)
declare void @telos_rt_int_to_text(ptr, i64)
declare i8 @telos_rt_int_parse(ptr, ptr)
declare i32 @telos_rt_tcp_set_idle_timeout(i64, i64, ptr)
declare ptr @telos_rt_store_new(ptr, ptr, ptr, i64, i64)
declare i64 @telos_rt_store_next_id(ptr)
declare void @telos_rt_store_put(ptr, i64, ptr)
declare i8 @telos_rt_store_get(ptr, i64, ptr)
declare i8 @telos_rt_store_remove(ptr, i64, ptr)
declare void @telos_rt_store_values(ptr, ptr)
declare i8 @telos_rt_store_update(ptr, i64, ptr, ptr, ptr)
declare ptr @malloc(i64)
declare void @free(ptr)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)

define internal void @telos.panic(ptr %bytes, i64 %len) cold noreturn {
entry:
  %text = alloca [24 x i8], align 8
  store ptr %bytes, ptr %text, align 8
  %lenp = getelementptr inbounds i8, ptr %text, i64 8
  store i64 %len, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %text, i64 16
  store i64 %len, ptr %capp, align 8
  call void @telos_rt_panic(ptr %text)
  unreachable
}

define internal void @telos.reserve(ptr %list, i64 %extra, i64 %size, i64 %align) {
entry:
  %lenp = getelementptr inbounds i8, ptr %list, i64 8
  %len = load i64, ptr %lenp, align 8
  %capp = getelementptr inbounds i8, ptr %list, i64 16
  %cap = load i64, ptr %capp, align 8
  %need = add i64 %len, %extra
  %fits = icmp sle i64 %need, %cap
  br i1 %fits, label %done, label %grow
grow:
  %double = mul i64 %cap, 2
  %big = icmp sgt i64 %double, %need
  %c1 = select i1 %big, i64 %double, i64 %need
  %small = icmp slt i64 %c1, 4
  %newcap = select i1 %small, i64 4, i64 %c1
  %bytes = mul i64 %newcap, %size
  %new = call ptr @telos.alloc(i64 %bytes, i64 %align)
  %old = load ptr, ptr %list, align 8
  %used = mul i64 %len, %size
  call void @llvm.memcpy.p0.p0.i64(ptr %new, ptr %old, i64 %used, i1 false)
  store ptr %new, ptr %list, align 8
  store i64 %newcap, ptr %capp, align 8
  br label %done
done:
  ret void
}

define internal void @telos.check_index(i64 %i, i64 %len) {
entry:
  %ok = icmp ult i64 %i, %len
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.index, i64 18)
  unreachable
fine:
  ret void
}

define internal void @telos.check_slice(i64 %i, i64 %j, i64 %len) {
entry:
  %a = icmp sge i64 %i, 0
  %b = icmp sle i64 %i, %j
  %c = icmp sle i64 %j, %len
  %ab = and i1 %a, %b
  %ok = and i1 %ab, %c
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.slice, i64 18)
  unreachable
fine:
  ret void
}

define internal void @telos.check_divisor(i64 %d) {
entry:
  %ok = icmp ne i64 %d, 0
  br i1 %ok, label %fine, label %bad
bad:
  call void @telos.panic(ptr @telos.msg.divide, i64 25)
  unreachable
fine:
  ret void
}

define internal void @telos.no_match() cold noreturn {
entry:
  call void @telos.panic(ptr @telos.msg.match, i64 12)
  unreachable
}

define internal ptr @telos.alloc(i64 %size, i64 %align) alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %limp = getelementptr inbounds i8, ptr %st, i64 8
  %lim = load ptr, ptr %limp, align 8
  %ci = ptrtoint ptr %cur to i64
  %li = ptrtoint ptr %lim to i64
  %am1 = sub i64 %align, 1
  %up = add i64 %ci, %am1
  %mask = sub i64 0, %align
  %start = and i64 %up, %mask
  %end = add i64 %start, %size
  %small = icmp ult i64 %size, 65536
  %fits = icmp ult i64 %end, %li
  %ok = and i1 %small, %fits
  br i1 %ok, label %fast, label %slow
fast:
  %skip = sub i64 %start, %ci
  %p = getelementptr i8, ptr %cur, i64 %skip
  %next = getelementptr i8, ptr %p, i64 %size
  store ptr %next, ptr %st, align 8
  ret ptr %p
slow:
  %q = call ptr @telos_rt_alloc(i64 %size, i64 %align)
  ret ptr %q
}

define internal i64 @telos.arena_mark() alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  ret i64 %pos
}

define internal i1 @telos.arena_due(i64 %mark, i64 %kept) alwaysinline {
entry:
  %pos = call i64 @telos.arena_mark()
  %above = sub i64 %pos, %mark
  %four = shl i64 %kept, 2
  %big = icmp sgt i64 %four, 65536
  %limit = select i1 %big, i64 %four, i64 65536
  %due = icmp sgt i64 %above, %limit
  ret i1 %due
}

define internal void @telos.arena_release(i64 %mark) alwaysinline {
entry:
  %st = call ptr @telos_rt_arena_state()
  %cur = load ptr, ptr %st, align 8
  %offp = getelementptr inbounds i8, ptr %st, i64 16
  %off = load i64, ptr %offp, align 8
  %ci = ptrtoint ptr %cur to i64
  %pos = add i64 %ci, %off
  %nothing = icmp sge i64 %mark, %pos
  br i1 %nothing, label %done, label %down
down:
  %basep = getelementptr inbounds i8, ptr %st, i64 24
  %base = load ptr, ptr %basep, align 8
  %bi = ptrtoint ptr %base to i64
  %bpos = add i64 %bi, %off
  %inside = icmp sge i64 %mark, %bpos
  br i1 %inside, label %chunk, label %slow
chunk:
  %back = sub i64 %mark, %bpos
  %new = getelementptr i8, ptr %base, i64 %back
  store ptr %new, ptr %st, align 8
  br label %done
slow:
  call void @telos_rt_arena_release(i64 %mark)
  br label %done
done:
  ret void
}

define i64 @"tests.small.SCALE"() {
entry:
  ret i64 2
}

define i64 @"tests.small.area"(ptr %p0) {
entry:
  %t1 = alloca [8 x i8], align 8
  %t13 = alloca [8 x i8], align 8
  %t4 = load i32, ptr %p0, align 4
  %t5 = icmp eq i32 %t4, 0
  br i1 %t5, label %L6, label %L3
L6:
  store i64 0, ptr %t1, align 8
  br label %L2
L3:
  %t8 = load i32, ptr %p0, align 4
  %t9 = icmp eq i32 %t8, 1
  br i1 %t9, label %L10, label %L7
L10:
  %t11 = getelementptr inbounds i8, ptr %p0, i64 8
  %t12 = load i64, ptr %t11, align 8
  store i64 %t12, ptr %t13, align 8
  %t14 = call i64 @"tests.small.SCALE"()
  %t15 = load i64, ptr %t13, align 8
  %t16 = mul i64 %t14, %t15
  %t17 = load i64, ptr %t13, align 8
  %t18 = mul i64 %t16, %t17
  store i64 %t18, ptr %t1, align 8
  br label %L2
L7:
  call void @telos.no_match()
  unreachable
L2:
  %t19 = load i64, ptr %t1, align 8
  ret i64 %t19
}

define void @"tests.small.label"(ptr %sret, ptr %p0) {
entry:
  %t3 = alloca [24 x i8], align 8
  %t6 = alloca [24 x i8], align 8
  %t8 = alloca [24 x i8], align 8
  %t16 = alloca [24 x i8], align 8
  %t19 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t1 = call ptr @telos.alloc(i64 24, i64 8)
  %t2 = getelementptr inbounds i8, ptr %p0, i64 8
  call void @"copy.Text"(ptr %t1, ptr %t2)
  store ptr %t1, ptr %t3, align 8
  %t4 = getelementptr inbounds i8, ptr %t3, i64 8
  store i64 1, ptr %t4, align 8
  %t5 = getelementptr inbounds i8, ptr %t3, i64 16
  store i64 1, ptr %t5, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t3, i64 24, i1 false)
  %t7 = load i64, ptr %p0, align 8
  call void @telos_rt_int_to_text(ptr %t8, i64 %t7)
  call void @telos.reserve(ptr %t6, i64 1, i64 24, i64 8)
  %t9 = getelementptr inbounds i8, ptr %t6, i64 8
  %t10 = load i64, ptr %t9, align 8
  %t11 = load ptr, ptr %t6, align 8
  %t12 = mul i64 %t10, 24
  %t13 = getelementptr inbounds i8, ptr %t11, i64 %t12
  call void @llvm.memcpy.p0.p0.i64(ptr %t13, ptr %t8, i64 24, i1 false)
  %t14 = add i64 %t10, 1
  %t15 = getelementptr inbounds i8, ptr %t6, i64 8
  store i64 %t14, ptr %t15, align 8
  store ptr @.str.0, ptr %t16, align 8
  %t17 = getelementptr inbounds i8, ptr %t16, i64 8
  store i64 1, ptr %t17, align 8
  %t18 = getelementptr inbounds i8, ptr %t16, i64 16
  store i64 1, ptr %t18, align 8
  call void @"copy.Text"(ptr %t19, ptr %t16)
  call void @telos_rt_text_join(ptr %t20, ptr %t6, ptr %t19)
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t20, i64 24, i1 false)
  ret void
}

define void @"tests.small.repeat"(ptr %sret, ptr %p0, i64 %p1) {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [24 x i8], align 8
  %t5 = alloca [24 x i8], align 8
  %t6 = alloca [24 x i8], align 8
  %t7 = alloca [8 x i8], align 8
  %t8 = alloca [24 x i8], align 8
  %t10 = alloca [8 x i8], align 8
  %t18 = alloca [24 x i8], align 8
  %t26 = alloca [24 x i8], align 8
  %t27 = alloca [8 x i8], align 8
  %t41 = alloca [1 x i8], align 1
  store i64 %p1, ptr %t1, align 8
  store ptr @telos.empty, ptr %t2, align 8
  %t3 = getelementptr inbounds i8, ptr %t2, i64 8
  store i64 0, ptr %t3, align 8
  %t4 = getelementptr inbounds i8, ptr %t2, i64 16
  store i64 0, ptr %t4, align 8
  call void @"copy.Text"(ptr %t5, ptr %t2)
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t5, i64 24, i1 false)
  store i64 0, ptr %t7, align 8
  %t9 = call i64 @telos.arena_mark()
  store i64 0, ptr %t10, align 8
  br label %L11
L11:
  %t15 = load i64, ptr %t7, align 8
  %t16 = load i64, ptr %t1, align 8
  %t17 = icmp slt i64 %t15, %t16
  br i1 %t17, label %L12, label %L14
L12:
  call void @telos_rt_text_concat(ptr %t18, ptr %t6, ptr %p0)
  call void @llvm.memcpy.p0.p0.i64(ptr %t6, ptr %t18, i64 24, i1 false)
  %t19 = load i64, ptr %t7, align 8
  %t20 = add i64 %t19, 1
  store i64 %t20, ptr %t7, align 8
  br label %L13
L13:
  %t21 = load i64, ptr %t10, align 8
  %t22 = call i1 @telos.arena_due(i64 %t9, i64 %t21)
  br i1 %t22, label %L23, label %L11
L23:
  call void @telos_rt_arena_scratch(i8 1)
  call void @"copy.Text"(ptr %t8, ptr %t6)
  call void @telos_rt_arena_scratch(i8 0)
  call void @telos_rt_arena_release(i64 %t9)
  call void @"copy.Text"(ptr %t6, ptr %t8)
  call void @telos_rt_arena_scratch_clear()
  %t24 = call i64 @telos.arena_mark()
  %t25 = sub i64 %t24, %t9
  store i64 %t25, ptr %t10, align 8
  br label %L11
L14:
  call void @"copy.List<Byte>"(ptr %t26, ptr %p0)
  store i64 0, ptr %t27, align 8
  %t28 = call i64 @telos.arena_mark()
  br label %L29
L29:
  %t33 = load i64, ptr %t27, align 8
  %t34 = getelementptr inbounds i8, ptr %t26, i64 8
  %t35 = load i64, ptr %t34, align 8
  %t36 = icmp slt i64 %t33, %t35
  br i1 %t36, label %L30, label %L32
L30:
  %t37 = load ptr, ptr %t26, align 8
  %t38 = mul i64 %t33, 1
  %t39 = getelementptr inbounds i8, ptr %t37, i64 %t38
  %t40 = load i8, ptr %t39, align 1
  store i8 %t40, ptr %t41, align 1
  %t42 = load i64, ptr %t7, align 8
  %t43 = add i64 %t42, 1
  store i64 %t43, ptr %t7, align 8
  br label %L31
L31:
  %t44 = load i64, ptr %t27, align 8
  %t45 = add i64 %t44, 1
  store i64 %t45, ptr %t27, align 8
  call void @telos.arena_release(i64 %t28)
  br label %L29
L32:
  call void @"copy.Text"(ptr %sret, ptr %t6)
  ret void
}

define void @"tests.small.shout"(ptr %sret, ptr %p0) {
entry:
  %t1 = alloca [24 x i8], align 8
  %t4 = alloca [24 x i8], align 8
  %t5 = alloca [24 x i8], align 8
  store ptr @.str.1, ptr %t1, align 8
  %t2 = getelementptr inbounds i8, ptr %t1, i64 8
  store i64 1, ptr %t2, align 8
  %t3 = getelementptr inbounds i8, ptr %t1, i64 16
  store i64 1, ptr %t3, align 8
  call void @"copy.Text"(ptr %t4, ptr %t1)
  call void @telos_rt_text_concat(ptr %t5, ptr %p0, ptr %t4)
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t5, i64 24, i1 false)
  ret void
}

define void @"tests.small.greet"(ptr %sret, ptr %p0) {
entry:
  %t1 = alloca [24 x i8], align 8
  call void @"tests.small.shout"(ptr %t1, ptr %p0)
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t1, i64 24, i1 false)
  ret void
}

define void @"tests.small.checked"(ptr %sret, i64 %p0) {
entry:
  %t1 = alloca [8 x i8], align 8
  %t7 = alloca [24 x i8], align 8
  %t10 = alloca [24 x i8], align 8
  %t11 = alloca [32 x i8], align 8
  %t12 = alloca [40 x i8], align 8
  %t16 = alloca [16 x i8], align 8
  %t19 = alloca [40 x i8], align 8
  store i64 %p0, ptr %t1, align 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = icmp slt i64 %t2, 0
  br i1 %t3, label %L4, label %L5
L4:
  store ptr @.str.2, ptr %t7, align 8
  %t8 = getelementptr inbounds i8, ptr %t7, i64 8
  store i64 8, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %t7, i64 16
  store i64 8, ptr %t9, align 8
  call void @"copy.Text"(ptr %t10, ptr %t7)
  call void @telos_rt_io_error_new(ptr %t11, ptr %t10)
  store i32 1, ptr %t12, align 4
  %t13 = getelementptr inbounds i8, ptr %t12, i64 8
  call void @llvm.memcpy.p0.p0.i64(ptr %t13, ptr %t11, i64 32, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t12, i64 40, i1 false)
  ret void
L14:
  br label %L6
L5:
  br label %L6
L6:
  %t15 = load i64, ptr %t1, align 8
  store i32 1, ptr %t16, align 4
  %t17 = getelementptr inbounds i8, ptr %t16, i64 8
  store i64 %t15, ptr %t17, align 8
  %t18 = call i64 @"tests.small.area"(ptr %t16)
  store i32 0, ptr %t19, align 4
  %t20 = getelementptr inbounds i8, ptr %t19, i64 8
  store i64 %t18, ptr %t20, align 8
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t19, i64 40, i1 false)
  ret void
}

define void @"tests.small.Echo.connect"(ptr %sret, i64 %p0) {
entry:
  %t1 = alloca [8 x i8], align 8
  %t2 = alloca [40 x i8], align 8
  %t11 = alloca [8 x i8], align 8
  %t12 = alloca [32 x i8], align 8
  %t14 = alloca [24 x i8], align 8
  %t17 = alloca [24 x i8], align 8
  %t19 = alloca [24 x i8], align 8
  %t20 = alloca [24 x i8], align 8
  %t22 = alloca [40 x i8], align 8
  %t23 = alloca [32 x i8], align 8
  store i64 %p0, ptr %t1, align 8
  call void @"tests.small.checked"(ptr %t2, i64 3)
  %t3 = load i32, ptr %t2, align 4
  %t4 = icmp eq i32 %t3, 1
  br i1 %t4, label %L5, label %L6
L5:
  store i32 1, ptr %sret, align 4
  %t7 = getelementptr inbounds i8, ptr %t2, i64 8
  %t8 = getelementptr inbounds i8, ptr %sret, i64 8
  call void @llvm.memcpy.p0.p0.i64(ptr %t8, ptr %t7, i64 32, i1 false)
  ret void
L6:
  %t9 = getelementptr inbounds i8, ptr %t2, i64 8
  %t10 = load i64, ptr %t9, align 8
  store i64 %t10, ptr %t11, align 8
  %t13 = load i64, ptr %t11, align 8
  store i64 %t13, ptr %t12, align 8
  store ptr @.str.3, ptr %t14, align 8
  %t15 = getelementptr inbounds i8, ptr %t14, i64 8
  store i64 4, ptr %t15, align 8
  %t16 = getelementptr inbounds i8, ptr %t14, i64 16
  store i64 4, ptr %t16, align 8
  call void @"copy.Text"(ptr %t17, ptr %t14)
  %t18 = getelementptr inbounds i8, ptr %t12, i64 8
  call void @llvm.memcpy.p0.p0.i64(ptr %t18, ptr %t17, i64 24, i1 false)
  call void @"tests.small.label"(ptr %t19, ptr %t12)
  call void @llvm.memcpy.p0.p0.i64(ptr %t20, ptr %t19, i64 24, i1 false)
  %t21 = load i64, ptr %t1, align 8
  %t24 = call i32 @telos_rt_tcp_write_all(i64 %t21, ptr %t20, ptr %t23)
  %t25 = icmp ne i32 %t24, 0
  br i1 %t25, label %L26, label %L27
L26:
  store i32 1, ptr %t22, align 4
  %t29 = getelementptr inbounds i8, ptr %t22, i64 8
  call void @llvm.memcpy.p0.p0.i64(ptr %t29, ptr %t23, i64 32, i1 false)
  br label %L28
L27:
  store i32 0, ptr %t22, align 4
  br label %L28
L28:
  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %t22, i64 40, i1 false)
  ret void
}

define internal void @"copy.List<Byte>"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @telos.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 1
  %t11 = call ptr @telos.alloc(i64 %t10, i64 1)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store ptr %t11, ptr %dst, align 8
  %t12 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t12, align 8
  %t13 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t13, align 8
  br label %L7
L7:
  ret void
}

define internal void @"copy.Text"(ptr %dst, ptr %src) {
entry:
  call void @llvm.memcpy.p0.p0.i64(ptr %dst, ptr %src, i64 24, i1 false)
  %t1 = getelementptr inbounds i8, ptr %src, i64 8
  %t2 = load i64, ptr %t1, align 8
  %t3 = load ptr, ptr %src, align 8
  %t4 = icmp sle i64 %t2, 0
  br i1 %t4, label %L5, label %L6
L5:
  store ptr @telos.empty, ptr %dst, align 8
  %t8 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 0, ptr %t8, align 8
  %t9 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 0, ptr %t9, align 8
  br label %L7
L6:
  %t10 = mul i64 %t2, 1
  %t11 = call ptr @telos.alloc(i64 %t10, i64 1)
  call void @llvm.memcpy.p0.p0.i64(ptr %t11, ptr %t3, i64 %t10, i1 false)
  store ptr %t11, ptr %dst, align 8
  %t12 = getelementptr inbounds i8, ptr %dst, i64 8
  store i64 %t2, ptr %t12, align 8
  %t13 = getelementptr inbounds i8, ptr %dst, i64 16
  store i64 %t2, ptr %t13, align 8
  br label %L7
L7:
  ret void
}
