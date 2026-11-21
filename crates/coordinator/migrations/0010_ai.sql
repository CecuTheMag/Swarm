-- Spec 0017: language models run only on devices that can hold them. A device reports its
-- GPU's largest buffer; a job whose model needs one says how large.
ALTER TABLE workers ADD COLUMN max_buffer_size bigint;
ALTER TABLE jobs ADD COLUMN min_gpu_buffer bigint;
